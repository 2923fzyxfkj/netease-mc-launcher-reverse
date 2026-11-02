# 02 客户端拓扑

> 这篇讲清楚：有几个进程、各自用什么技术栈、TLS 走哪条路。**抓包策略完全由这张图决定。**

## 一张图

```text
┌─────────────────────────────────────────────────────────┐
│ 网易发烧游戏  FeverGamesInstaller.exe                    │
│  Qt + QML + CEF（79 MB）                                 │
│   ① 账号登录：CEF 网页 + 原生 UniSDK                      │
│   ② 登录态明文写入注册表                                  │
│   ③ 通过 WM_COPYDATA 与本进程内的 LHMW_FG_Main 窗口交互   │
└──────────────────┬──────────────────────────────────────┘
                   │ CreateProcessW
                   │ 命令行：--start_from_launcher=1
                   ↓
┌─────────────────────────────────────────────────────────┐
│ MC 启动器  WPFLauncher.exe（.NET 4.5.2 / WPF，32 位）      │
│   ④ 通过本地 IPC 索取 ticket                             │
│   ⑤ 调 UniSDK 换取 MC 会话（user-token）                  │
│   ⑥ 调用联机大厅 HTTP API                                 │
└──────────────────┬──────────────────────────────────────┘
                   │ HTTPS
                   ↓
┌─────────────────────────────────────────────────────────┐
│ x19apigatewayobt.nie.netease.com:443   业务 API          │
│ x19obtcore.nie.netease.com:8443        登录/会话          │
└─────────────────────────────────────────────────────────┘
```

## 进程与技术栈对照

| 进程 | 技术 | TLS 实现 | 抓包可行性 |
|---|---|---|---|
| `FeverGamesInstaller.exe` | Qt + QML + CEF | 混合：CEF 用 BoringSSL，原生用 Schannel | 差 |
| `FeverGamesWeb.exe` | CEF 渲染/GPU 进程 | BoringSSL | 差 |
| `WPFLauncher.exe` | .NET Framework 4.5.2 / WPF | **Schannel（`secur32`）** | **好** |
| `Minecraft.Windows.exe` | 游戏本体 | RakNet（UDP） | 不需要 |

## 为什么这个拓扑很重要

它直接否掉了三种常见的抓包思路：

| 思路 | 为什么不行 |
|---|---|
| 设系统代理 + mitmproxy | `WPFLauncher.exe.config` 里 `defaultProxy enabled="false"` |
| 设 WinHTTP 代理 | WinHTTP 与 WinINet 是**两套独立配置**，且登录走的是 WinHTTP |
| 装 CA 到系统证书库抓 CEF | 登录页在 CEF 里，走 Chromium 自带的 BoringSSL，**不读系统证书库** |

同时也提示了正确方向：

> `WPFLauncher.exe` 是 32 位 .NET 程序，HTTP 走 Windows 原生 Schannel。
> **明文必经 `secur32.dll` 的 `EncryptMessage` / `DecryptMessage`。**

## 版本信息（实测）

```text
WPFLauncher.exe        1.15.31.20647  (x86)
网易发烧游戏            1.18.45.8
UniSDK                 sdk_version 4.10.0
```

## 关键文件位置

| 内容 | 路径 |
|---|---|
| MC 启动器 | `C:\FeverApps\MC\WPFLauncher.exe` |
| 发烧游戏 | `C:\Program Files\FeverGames\<版本>\FeverGamesInstaller.exe` |
| UniSDK | `C:\FeverApps\MC\NtUniSdk*.dll` |
| CEF 缓存 | `%LOCALAPPDATA%\FeverGames\FeverGamesInstaller\cache\CefCache` |
| 登录态 | `HKCU\Software\FeverGames\FeverGamesInstaller\data\token` |

## 下一步

- 想开始抓包 → [03 抓包环境与选型](03-capture-setup.md)
- 想看具体 hook 代码 → [04 Schannel Hook](04-schannel-hook.md)
