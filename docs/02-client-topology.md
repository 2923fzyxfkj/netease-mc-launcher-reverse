# 02 客户端拓扑

> 这篇讲清楚：有几个进程、各自用什么技术栈、TLS 走哪条路。**抓包策略完全由这张图决定。**

## 进程与数据流

```mermaid
flowchart TD
    FG["网易发烧游戏<br/>FeverGamesInstaller.exe<br/>Qt + QML + CEF (79 MB)"]
    WT["MC 启动器<br/>WPFLauncher.exe<br/>.NET 4.5.2 / WPF (32 位)"]
    API["业务 API<br/>x19apigatewayobt.nie.netease.com:443"]
    CORE["登录 / 会话<br/>x19obtcore.nie.netease.com:8443"]
    REG[("Windows 注册表<br/>HKCU\\Software\\FeverGames<br/>\\FeverGamesInstaller\\data\\token")]

    FG -->|"① 账号登录（CEF 网页 + 原生 UniSDK）"| FG
    FG -->|"② 登录态明文持久化"| REG
    FG -->|"③ 本进程内 WM_COPYDATA<br/>与 LHMW_FG_Main 窗口交互"| FG
    FG -->|"CreateProcessW<br/>--start_from_launcher=1"| WT
    WT -->|"④ 本地 IPC 索取 ticket"| FG
    WT -->|"⑤ 调 UniSDK 换取 user-token"| CORE
    WT -->|"⑥ HTTPS 建房 / 心跳"| API
    REG -.->|"可离线读取"| API
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

```mermaid
flowchart LR
    A["设系统代理 + mitmproxy"] --> A1["❌ defaultProxy=false"]
    B["设 WinHTTP 代理"] --> B1["❌ 与 WinINet 是两套配置"]
    C["装 CA 抓 CEF"] --> C1["❌ BoringSSL 不读系统证书库"]
    D["hook secur32"] --> D1["✅ 明文必经之路"]
```

同时也提示了正确方向：

> `WPFLauncher.exe` 是 32 位 .NET 程序，HTTP 走 Windows 原生 Schannel。
> **明文必经 `secur32.dll` 的 `EncryptMessage` / `DecryptMessage`。**

## 版本信息（实测）

| 组件 | 版本 |
|---|---|
| `WPFLauncher.exe` | `1.15.31.20647`（x86） |
| 网易发烧游戏 | `1.18.45.8` |
| UniSDK | `sdk_version 4.10.0` |

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
