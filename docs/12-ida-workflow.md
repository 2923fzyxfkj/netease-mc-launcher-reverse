# 12 IDA 工作流备忘

> IDA 7.6 在 Windows 上的几个坑，以及批量分析的可用姿势。

## 最大的坑：`idat64` 启动即失败

现象：

```text
IDAPython: error executing init.py:
No module named 'imp'
```

原因：IDA 7.6 的 IDAPython 依赖 `imp` 模块，
而 **Python 3.12 起移除了 `imp`**。`idapyswitch` 默认会选最新的 Python（如 3.14），于是必挂。

修复：

```powershell
# 查看可用 Python
& "<IDA 目录>\idapyswitch.exe" -r

# 指定 Python 3.9
& "<IDA 目录>\idapyswitch.exe" -s "C:\...\Python39\python3.dll"
```

## 用 GUI 版的批处理模式

`idat64.exe`（文本版）即使修好 Python 也可能静默退出（退出码 1，无日志）。
**改用 GUI 版的 `-B -A` 参数可以正常产出**：

```powershell
& "<IDA 目录>\ida64.exe" -B -A "<目标文件>"
```

产物（与目标文件同目录）：

| 文件 | 内容 |
|---|---|
| `<目标>.asm` | 完整反汇编文本 |
| `<目标>.i64` | IDA 数据库 |

实测：390 KB 的 `FeverGamesLauncher.exe` 约 2 分钟；
4.4 MB 的 `NtUniSdkBase.dll` 约 5 分钟。

## 为什么不要用 IDA 逆向 .NET

`WPFLauncher.exe` 是 .NET 程序集，IDA 对 .NET 的支持很弱。
真要看逻辑应该用 dnSpy / ILSpy。

但注意：`WPFLauncher` 的业务字符串是 `!x19sign!` 加密的，
静态反编译基本看不到明文，**动态 hook 更有效**。

## 在 asm 里找线索的套路

### 找字符串引用

```powershell
Select-String -LiteralPath "<目标>.asm" -Pattern 'sessionid|SAUTH_JSON|ticket' |
  Select-Object -First 25
```

### 看某个地址附近的代码

```powershell
$lines = Get-Content -LiteralPath "<目标>.asm"
for ($i = 69700; $i -lt 69920; $i++) { "{0,7}: {1}" -f ($i+1), $lines[$i] }
```

### 找调用了某个 API 的所有位置

```powershell
Select-String -LiteralPath "<目标>.asm" -Pattern 'call\s+cs:SendMessageTimeoutW' -Context 12,0
```

## 这次逆向用 IDA 得到的三个结论

| 结论 | 依据 |
|---|---|
| ticket 走 `WM_COPYDATA` 到 `LHMW_FG_Main` | `FindWindowExW(HWND_MESSAGE)` + `SendMessageTimeoutW(0x4A)` |
| IPC 组件叫 `ncgipc` | 源码路径 `components\ipc\NCGIPCMessage.cpp` |
| 有一张 496 字节的密钥表 | `qword` 地址 + 滚动 XOR 循环 |

## 提取 PE 里的数据（按 RVA）

有时需要把 IDA 里看到的某个地址的数据原样 dump 出来：

```python
import struct

PATH = r"<目标文件>"
VA = 0x14003B860
BASE = 0x140000000
LEN = 496

data = open(PATH, "rb").read()
pe = struct.unpack_from("<I", data, 0x3C)[0]
nsec = struct.unpack_from("<H", data, pe + 6)[0]
opt = pe + 24
magic = struct.unpack_from("<H", data, opt)[0]
sec_off = opt + (240 if magic == 0x20B else 224)

rva = VA - BASE
for i in range(nsec):
    off = sec_off + i * 40
    vsize, vaddr = struct.unpack_from("<II", data, off + 8)
    rawsize, rawptr = struct.unpack_from("<II", data, off + 16)
    if vaddr <= rva < vaddr + max(vsize, rawsize):
        file_off = rawptr + (rva - vaddr)
        open("dump.bin", "wb").write(data[file_off:file_off + LEN])
        break
```

## 常用快捷键

| 操作 | 快捷键 |
|---|---|
| 跳转地址 | `G` |
| 交叉引用 | `X` |
| 重命名 | `N` |
| 反编译（需 Hex-Rays） | `F5` |
| 导出伪代码 | 文件菜单 → Produce file → Create C file |

## 相关

- 用 IDA 得到的 IPC 结论 → [11 本地 IPC 与 ticket](11-ipc-ticket.md)
- 还没攻克的加密 → [09 登录第二步](09-authentication-otp.md)
