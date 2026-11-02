# 网易我的世界登录器逆向笔记

> 记录一次对网易《我的世界》中国版电脑端的协议逆向：从 `sessionId`、`user-token` 到联机大厅 API，全程由一个 AI Agent（DeepSeek + DeepSeek Harness）探索，人类只负责按「允许」。

[![License](https://img.shields.io/badge/license-MIT-blue)](LICENSE.md)

## 这是什么

这是一份**逆向过程笔记**，不是可运行的作弊工具。它记录了我们如何从「完全不知道能不能做」推进到「只差一道加密握手」，包括：

- 为什么常规抓包对网易客户端全部失效，以及怎么绕过
- 联机大厅的建房与心跳协议（已完全破解，纯 HTTP）
- 登录流程的三步链路（第一步明文可重放，第二步加密未攻克）
- 登录态在注册表里的明文存储位置

**给谁看**：对网易客户端协议、Windows 抓包、Frida 注入感兴趣的研究者。

## 结论速览

| 环节 | 状态 | 说明 |
|---|---|---|
| 建房 `POST /online-lobby-room` | 已破解 | 纯 HTTP，可复现 |
| 心跳 `POST /online-lobby-member-tick` | 已破解 | 30 秒周期，可复现 |
| 会话凭证 `user-token` | 受限 | 随客户端会话失效，关掉启动器即 `code:10` |
| 登录第一步 `POST /login-otp` | 已破解 | 明文 JSON，已验证可重放 |
| 登录第二步 `POST /authentication-otp` | 未攻克 | 请求与响应均为端到端加密 |
| 本地登录态 | 已定位 | 明文持久化在注册表 |

## 客户端拓扑

```text
网易发烧游戏 FeverGamesInstaller.exe (Qt + QML + CEF)
   │ ① 账号登录（CEF 网页 + 原生 UniSDK）
   │ ② 登录态明文写入注册表
   ↓ CreateProcessW
WPFLauncher.exe --start_from_launcher=1 (.NET 4.5.2 WPF)
   │ ③ 通过本地 WM_COPYDATA IPC 索取 ticket
   │ ④ 调 UniSDK 换取 MC 会话
   ↓
x19apigatewayobt.nie.netease.com（联机大厅 HTTP API）
```

## 为什么常规抓包一定失败

四个坑，按踩到的顺序：

1. `WPFLauncher.exe.config` 里 `defaultProxy enabled="false"`，系统代理对它无效。
2. WinHTTP 与 WinINet 是两套独立代理配置，登录走 WinHTTP。
3. 登录页在 CEF 里，走 Chromium 自带的 BoringSSL，不经过系统证书库。
4. 客户端没有加载 `libcurl` / `libssl`，hook 它们是白费。

## 正确的抓法

启动器用 Windows 原生 Schannel，明文必经 `secur32.dll`：

```javascript
var m = Process.findModuleByName("secur32.dll");

// 注意：两个函数的参数顺序不同
// EncryptMessage(ctx, fQOP, pMessage, seq)   → pMessage 是 a[2]
// DecryptMessage(ctx, pMessage, seq, pfQOP)  → pMessage 是 a[1]
Interceptor.attach(m.findExportByName("EncryptMessage"), {
  onEnter: function (a) { /* dump a[2] 指向的 SecBufferDesc */ }
});
Interceptor.attach(m.findExportByName("DecryptMessage"), {
  onEnter: function (a) { /* pMessage 是 a[1]，别搞错 */ }
});
```

`SecBufferDesc` 在 32 位进程下的布局（WPFLauncher 是 32 位，这点坑了很久）：

```text
SecBufferDesc: [ulVersion 4B] [cBuffers 4B] [pBuffers 4B]
SecBuffer:     [cbBuffer 4B]  [BufferType 4B] [pvBuffer 4B]
SECBUFFER_DATA = 1
```

## 联机大厅协议

### 建房

```http
POST /online-lobby-room HTTP/1.1
X_TRACE_ID: <32 位随机字母数字>
user-id: <数字账号 ID>
user-token: <16 位，末位恒为 '1'>
User-Agent: WPFLauncher/1.15.31.20647
Host: x19apigatewayobt.nie.netease.com

{"room_name":"...","max_count":2,"visibility":0,"res_id":"<地图 ID>","save_id":"","password":""}
```

> [!WARNING]
> 绝对不能带 `Content-Type: application/x-www-form-urlencoded`。Python 的 `urllib` 会自动补这个头，服务端按表单解析，body 全空，返回 `code:4 参数为空`。

### 心跳保活

```http
POST /online-lobby-member-tick
```

```json
{"room_id":"<房间 ID>","password":null,"check_visibilily":false}
```

- 周期 30 秒
- 字段名就是 `check_visibilily`（官方拼写错误），不要修正
- 成功响应：`{"code":0,"message":"正常返回","details":"","entity":null}`

### 错误码

| code | 含义 | 常见原因 |
|---|---|---|
| 0 | 成功 | |
| 4 | 参数为空 | body 未被解析（Content-Type 陷阱） |
| 10 | 请先登录 | token 随客户端会话失效 |
| 12 | 参数错误 | token 无效 |
| 13 | 操作失败 | 高频重复调用 |
| 22 | 您的帐号在另一处登录 | 同账号互斥 |

## 登录协议

```text
POST /login-otp           明文 JSON，已破解
      ↓ 返回 otp_token / aid
POST /authentication-otp  加密二进制，未攻克
      ↓ 返回 16 位 user-token
POST /interconn/web/game-play-v2/login-start
```

Host 是 `x19obtcore.nie.netease.com:8443`，注意不是 `x19apigatewayobt`。

`login-otp` 的请求体是明文 JSON，核心字段：

```json
{"sauth_json":"{\"gameid\":\"x19\",\"login_channel\":\"netease\",\"sdkuid\":\"...\",\"sessionid\":\"1-...\",\"access_token\":\"<JWT>\"}"}
```

`sessionid` 去掉 `1-` 前缀后是 base64 的 JSON：

```json
{"prs":30,"lt":1793625548,"odi":"<deviceid>","s":"[mp]-<32 字符>-<64 hex>","u":"<sdkuid>","si":"<40 hex>","g_i":"<gameid>","sa":3}
```

`s` 字段是服务端签发的签名，不是本地生成——所以不需要逆向签名算法。

## 已知限制

- **同账号互斥**：同一账号同一时间只能一处在线，并发会 `code:22`。
- **token 与会话绑定**：客户端退出即注销，实测每 5 秒发请求也续不住。
- **第二步加密未解**：`authentication-otp` 请求与响应均为高熵密文。
- **合规风险**：模拟客户端协议可能违反用户协议，仅供学习研究。

## 仓库结构

```text
README.md                  本文件
docs/protocol.md           协议速查（可直接照做）
docs/reverse-engineering.md 完整逆向记录与踩坑清单
LICENSE.md                 MIT
```

## 贡献

欢迎补充 `authentication-otp` 的加密分析。请先阅读 `docs/reverse-engineering.md`，避免重复踩坑。

## 许可

[MIT](LICENSE.md)

---

本项目为协议学习与研究用途。所有账号、token、设备标识均已脱敏。
