# 03 抓包环境与选型

> 这篇讲清楚：为什么常规方案全废，最后用什么工具链，以及环境准备清单。

## 四种方案的全部试错记录

### 方案 A：系统代理 + mitmproxy（失败）

```text
设系统代理 127.0.0.1:8081
装 mitmproxy CA 到「当前用户 → 受信任的根证书颁发机构」
```

**结果**：抓到了 CEF 的页面请求（`fever.163.com`、`loadingbaycn.webapp.163.com`），
但**一个登录请求都没有**。

**原因**：`WPFLauncher.exe.config` 里写着

```xml
<system.net>
  <defaultProxy enabled="false" useDefaultCredentials="false">
    <proxy />
  </defaultProxy>
</system.net>
```

### 方案 B：WinHTTP 代理（失败）

发现登录可能走 WinHTTP（它与 WinINet 是两套独立配置）：

```powershell
netsh winhttp set proxy 127.0.0.1:8081
```

**结果**：仍然没有登录请求。CEF 与 SDK 的请求都不在这条链路上。

### 方案 C：抓 CEF 的远程调试端口（不可用）

```powershell
netstat -ano | Select-String 'LISTENING'   # 没找到 CEF 的 --remote-debugging-port
```

客户端没有开启调试端口。

### 方案 D：Frida hook Schannel（成功）

这是最终采用的方案，详见 [04 Schannel Hook](04-schannel-hook.md)。

## 最终工具链

| 工具 | 用途 | 备注 |
|---|---|---|
| **Frida 17** | 注入 + hook | 需要管理员权限 |
| **Wireshark / tshark** | 辅助确认连接与 SNI | 注意选对网卡 |
| **IDA 7.6** | 静态分析 | `idat64` 在 Python 3.14 下会挂，见 [12](12-ida-workflow.md) |
| **Python 3.x** | 写抓包脚本、重放请求 | `frida` 模块需装在同一个解释器里 |

## 环境准备清单

```bash
# 1. Python 侧
pip install frida-tools frida

# 2. 确认 Frida 能看到目标进程
frida-ps | findstr /i "WPFLauncher FeverGames"

# 3. 确认 IDA 可用（GUI 版批处理模式）
"<IDA 目录>\ida64.exe" -B -A <目标文件>
```

## 关于网卡选择

一个容易忽略的坑：如果机器同时有有线与无线网卡，流量只走其中一个。

```powershell
ipconfig    # 找到真正有 IP 的那块网卡
"C:\Program Files\Wireshark\tshark.exe" -i 4 -a duration:10 -q -z io,stat,0
```

抓 0 个包通常不是权限问题，而是**选错了网卡**。

## 关于代理软件

如果机器上跑着 Clash / V2Ray 之类的代理：

- 它们可能开了 TUN 模式，会把流量劫持到虚拟网卡
- 抓包时建议**临时退出**，否则看到的都是代理进程的流量
- 客户端本身也可能配了自己的加速器（如 UU），同样会干扰

## 下一步

- 具体怎么 hook → [04 Schannel Hook](04-schannel-hook.md)
- 怎么抓「启动瞬间」的请求 → [05 进程启动瞬间注入](05-process-injection.md)
