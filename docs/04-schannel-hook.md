# 04 Schannel Hook

> 这篇讲清楚：为什么 hook `secur32`、两个函数的参数陷阱、`SecBufferDesc` 结构，以及完整可用的 Frida 脚本。

## 原理

`WPFLauncher.exe` 是 .NET 程序，它的 HTTPS 请求最终落到 Windows 原生 Schannel：

```text
.NET HttpWebRequest / WebClient
      ↓ SslStream
secur32.dll!InitializeSecurityContext   （握手）
secur32.dll!EncryptMessage              （发送明文 → 密文）
secur32.dll!DecryptMessage              （接收密文 → 明文）
```

**只要我们 hook 住这两个函数，就能在加密前/解密后拿到明文。**

## 两个致命细节

### 细节一：参数顺序不同

```c
// EncryptMessage(ctx, fQOP, pMessage, seq)
//   pMessage 是第 3 个参数 → Frida 里是 a[2]

// DecryptMessage(ctx, pMessage, seq, pfQOP)
//   pMessage 是第 2 个参数 → Frida 里是 a[1]
```

把 `DecryptMessage` 的 `a[2]` 当成 `pMessage` 会读到 `MessageSeqNo`（一个小整数），
表现为 `access violation accessing 0x4`。

### 细节二：32 位结构体布局

`WPFLauncher.exe` 是 **x86**，结构体是 12 字节：

```text
SecBufferDesc（32 位）
  +0  ulVersion   ULONG   (4B)
  +4  cBuffers    ULONG   (4B)
  +8  pBuffers    PSecBuffer (4B)

SecBuffer（32 位，每个 12 字节）
  +0  cbBuffer    ULONG   (4B)
  +4  BufferType  ULONG   (4B)
  +8  pvBuffer    PVOID   (4B)

BufferType: SECBUFFER_DATA = 1
```

按 64 位布局（`readU64` + 偏移 8/16）去读会拿到垃圾值。

## 完整脚本

```javascript
var m = Process.findModuleByName("secur32.dll");
if (!m) {
  // .NET 可能还没加载它，需要轮询等待
}

function snapshot(descPtr) {
  var arr = [];
  try {
    var cBuffers = descPtr.add(4).readU32();
    var pBuffers = descPtr.add(8).readPointer();
    for (var i = 0; i < cBuffers && i < 8; i++) {
      var b = pBuffers.add(i * 12);          // 32 位 SecBuffer 步长 12
      arr.push({
        cb: b.readU32(),
        type: b.add(4).readU32(),
        pv: b.add(8).readPointer()
      });
    }
  } catch (e) {}
  return arr;
}

function emit(arr, tag) {
  for (var i = 0; i < arr.length; i++) {
    var x = arr[i];
    if ((x.type & 0xffff) === 1 && x.cb > 0 && x.cb < 65536 && x.pv && !x.pv.isNull()) {
      var s = "";
      var bytes = new Uint8Array(x.pv.readByteArray(Math.min(x.cb, 16384)));
      for (var j = 0; j < bytes.length; j++) {
        s += (bytes[j] >= 32 && bytes[j] < 127) ? String.fromCharCode(bytes[j]) : ".";
      }
      send({ dir: tag, len: x.cb, text: s });
    }
  }
}

var enc = m && m.findExportByName("EncryptMessage");
var dec = m && m.findExportByName("DecryptMessage");

if (enc) {
  Interceptor.attach(enc, {
    onEnter: function (a) { emit(snapshot(a[2]), "REQ"); }   // a[2] = pMessage
  });
}

if (dec) {
  Interceptor.attach(dec, {
    onEnter: function (a) { this.bufs = snapshot(a[1]); },    // a[1] = pMessage
    onLeave: function () { emit(this.bufs, "RESP"); }
  });
}
```

## 为什么接收方向要在 onLeave 读

- `onEnter` 时数据还是密文
- `onLeave` 时 `DecryptMessage` 已就地解密

但**指针本身**必须在 `onEnter` 就快照下来，因为 `onLeave` 时栈上的参数描述符可能已失效。

## 响应解压

抓到的响应体通常是 gzip。正确切分方式：

```python
import gzip, re

i = data.find(b"HTTP/1.")
head, _, body = data[i:].partition(b"\r\n\r\n")
m = re.search(rb"Content-Length:\s*(\d+)", head)
if m:
    body = body[:int(m.group(1))]
plain = gzip.decompress(body).decode("utf-8", "replace")
```

> [!WARNING]
> 不要直接搜索 `\x1f\x8b` 魔数去找 gzip 起点——TLS 记录里有大量误匹配，
> 表现为 `BadGzipFile: Not a gzipped file` 或 `CRC check failed`。

## 常见现象对照

| 现象 | 原因 |
|---|---|
| `enc=false dec=false` | 注入太早，.NET 还没加载 `secur32.dll`，需轮询等待 |
| `access violation accessing 0x4` | 把 `DecryptMessage` 的 `a[2]` 当成了 `pMessage` |
| 只抓到 REQ 没有 RESP | 响应还没返回，或 `onLeave` 里又去读了失效的指针 |
| 中文 body 变乱码 | 用了 ASCII 转义，属正常，按 `Content-Length` 截取原始字节再解压 |

## 下一步

- 抓不到启动瞬间的请求 → [05 进程启动瞬间注入](05-process-injection.md)
- 明文拿到了，协议是什么 → [06 建房](06-lobby-room-api.md)
