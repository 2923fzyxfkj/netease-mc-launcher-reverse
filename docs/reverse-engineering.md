# LRAO 逆向交接文档

> 这份文档记录了一次完整的逆向过程：从「完全不知道能不能做」到「只差一道加密握手」。
> **目的：让后来者不必重走同样的弯路。**

## 0. 项目目标

让网易《我的世界》中国版联机大厅的房间**保持常在线**，最终希望在**无图形环境的服务器**上无人值守运行。

## 1. 结论速览

| 环节 | 状态 | 说明 |
|---|---|---|
| 建房 `POST /online-lobby-room` | ✅ 完全破解 | 纯 HTTP，可复现 |
| 心跳 `POST /online-lobby-member-tick` | ✅ 完全破解 | 30 秒周期，可复现 |
| 会话凭证 `user-token` | ⚠️ 随客户端会话失效 | 关掉启动器即 `code:10` |
| 登录第一步 `POST /login-otp` | ✅ 明文协议，可重放 | 返回 `otp_token` / `aid` |
| 登录第二步 `POST /authentication-otp` | ❌ 端到端加密 | 最后一道墙，未攻克 |
| 本地登录态存储 | ✅ 明文持久化在注册表 | 关掉启动器仍可读 |

**一句话**：协议层已经通了，只差登录第二步的加密握手。

## 2. 客户端拓扑

```
网易发烧游戏 (FeverGamesInstaller.exe, Qt+QML+CEF, 79MB)
   │  ① 账号登录（CEF 内网页 + 原生 UniSDK）
   │  ② 生成/下发登录态，明文存注册表
   ↓ CreateProcessW
WPFLauncher.exe  --start_from_launcher=1   (MC 启动器, .NET 4.5.2 WPF)
   │ ③ 通过本地 IPC 向发烧游戏索取 ticket
   │ ④ 调 UniSDK 换取 MC 会话
   ↓
联机大厅 HTTP API (x19apigatewayobt.nie.netease.com)
```

### 2.1 各进程技术栈

| 进程 | 技术 | TLS 实现 | 抓包可行性 |
|---|---|---|---|
| `FeverGamesInstaller.exe` | Qt + QML + CEF | 混合（CEF=BoringSSL，原生=Schannel） | 差（多路径） |
| `FeverGamesWeb.exe` | CEF 渲染进程 | BoringSSL | 差 |
| `WPFLauncher.exe` | .NET Framework 4.5.2 / WPF | **Schannel (`secur32`)** | **好** |
| `Minecraft.Windows.exe` | 游戏本体 | RakNet(UDP) | 不需要 |

## 3. 抓包方法论（重点）

### 3.1 为什么常规抓包失败

1. `WPFLauncher.exe.config` 里 **`defaultProxy enabled="false"`** → 系统代理对它无效
2. WinHTTP 与 WinINet 代理是**两套独立配置**，登录走 WinHTTP
3. CEF 登录请求不走系统代理
4. 没有 `libcurl` / `libssl`（不要浪费时间 hook 它们）

### 3.2 正确的做法：Frida hook Schannel

启动器用 Windows 原生 Schannel，明文必经 `secur32.dll`：

```js
var m = Process.findModuleByName("secur32.dll");
var enc = m.findExportByName("EncryptMessage");   // 发送方向
var dec = m.findExportByName("DecryptMessage");   // 接收方向

// 注意：两个函数参数顺序不同！
// EncryptMessage(ctx, fQOP, pMessage, seq)          → pMessage 是 a[2]
// DecryptMessage(ctx, pMessage, seq, pfQOP)         → pMessage 是 a[1]
```

`SecBufferDesc` 在 32 位进程下布局（**WPFLauncher 是 32 位，这一点坑了很久**）：

```
SecBufferDesc: [ulVersion(4)] [cBuffers(4)] [pBuffers(4)]
SecBuffer:     [cbBuffer(4)] [BufferType(4)] [pvBuffer(4)]
SECBUFFER_DATA = 1
```

### 3.3 抓取"进程启动瞬间"的请求

关键是**在子进程创建时立即注入**：

```python
# host: hook CreateProcessW，onLeave 读 PROCESS_INFORMATION.dwProcessId
#       注意回调里只入队，注入必须在主线程做，否则 frida.attach 会死锁
# child: 轮询等待 secur32.dll 加载后再挂钩
#        注入时机太早时 .NET 还没加载 secur32，会得到 enc=false
```

**教训记录**：
- 不要在 Frida 的 Python 回调线程里调用 `frida.attach()`（重入死锁）
- 不要 hook `CREATE_SUSPENDED` 的进程（DllMain 不运行，attach 死锁）
- `spawn()` 模式启动 WPFLauncher **不会**触发登录 IPC（缺父进程上下文），必须走正常启动

### 3.4 HTTP 响应解压

TLS 记录里同时含 HTTP 头和 gzip body，正确切分方式：

```python
i = data.find(b"HTTP/1.")
head, _, body = data[i:].partition(b"\r\n\r\n")
cl = re.search(rb"Content-Length:\s*(\d+)", head)
body = body[:int(cl.group(1))]
plain = gzip.decompress(body)
```

**不要**直接搜索 `\x1f\x8b` 魔数——TLS 记录里有大量误匹配。

## 4. 联机大厅 API

### 4.1 建房

```http
POST /online-lobby-room HTTP/1.1
X_TRACE_ID: <32位随机字母数字>
user-id: <数字账号ID>
user-token: <16位，末位恒为 '1'>
User-Agent: WPFLauncher/1.15.31.20647
Host: x19apigatewayobt.nie.netease.com

{"room_name":"...","max_count":2,"visibility":0,"res_id":"<地图ID>","save_id":"","password":""}
```

**坑**：绝对不能带 `Content-Type: application/x-www-form-urlencoded`。
Python `urllib` 会自动补这个头，导致服务端按表单解析 → `code:4 参数为空` / `code:13 操作失败`。
用 `http.client` 手动控制，或者干脆不发 `Content-Type`。

### 4.2 心跳（保活）

```http
POST /online-lobby-member-tick
```

```json
{"room_id":"<房间ID>","password":null,"check_visibilily":false}
```

- 周期 **30 秒**（实测）
- 字段名就是 `check_visibilily`（官方拼写错误），**不要修正**
- 成功：`{"code":0,"message":"正常返回","details":"","entity":null}`

### 4.3 错误码

| code | 含义 | 原因 |
|---|---|---|
| 0 | 成功 | |
| 4 | 参数为空 | body 没被解析（Content-Type 问题） |
| 10 | 请先登录 | token 随客户端会话失效 |
| 12 | 参数错误 | token 伪造/无效 |
| 13 | 操作失败 | 高频重复调用 |
| 22 | 您的帐号在另一处登录 | **同账号互斥** |

## 5. 登录态存储（关键发现）

### 5.1 明文注册表

```
HKCU\Software\FeverGames\FeverGamesInstaller\data\token    (REG_BINARY, Base64(JSON))
HKCU\Software\FeverGames\FeverGamesInstaller\data\sdkuid
HKCU\Software\FeverGames\FeverGamesInstaller\data\settingSocial
HKCU\Software\FeverGames\FeverGamesInstaller\game\<gid>\StartupParams
```

`data\token` 解码后：

```json
{
  "account_id": "<数字>",
  "account_name": "<邮箱脱敏>",
  "name": "<昵称>",
  "userId": "<8位ID>",
  "token": "<64位>",
  "type": 10,
  "newUser": false
}
```

**这是整个项目最重要的发现**：登录态不需要启动器运行、不需要图形会话即可读取。

### 5.2 二进制 IPC（ticket 传递）

`FeverGamesLauncher.exe` 通过 **`WM_COPYDATA (0x4A)`** 向窗口类 `LHMW_FG_Main` 索取 ticket：

```
窗口类名格式：LHMW_FG_p<pid>_i<n> / LHMW_FG_client<sid>_i<n>
消息：WM_COPYDATA
数据：含明文标识（NAME / RSA1）的二进制，其余为加密
空字符串 → "can not get ticket from Fever Games, you may need to check login status in Fever Games"
```

相关 API：`FindWindowExW(HWND_MESSAGE, ...)`、`SendMessageTimeoutW`、`CreateEventW`、`CreateThread`

**未解**：IPC 数据的加密算法（不是简单的滚动 XOR，试过 1600 种偏移组合均失败）。

## 6. 登录协议（最后一道墙）

### 6.1 完整链路

```
POST /login-otp            (明文 JSON)  ✅
    ↓ 返回 otp_token / aid
POST /authentication-otp   (加密二进制) ❌
    ↓ 返回 16 位 user-token
POST /interconn/web/game-play-v2/login-start  (带 user-token) ✅ 返回 code:0
```

Host：`x19obtcore.nie.netease.com:8443`（**注意不是** `x19apigatewayobt`）

### 6.2 login-otp 请求（明文，已验证可重放）

```json
{"sauth_json":"{\"gameid\":\"x19\",\"login_channel\":\"netease\",
\"app_channel\":\"a50_sdk_cn\",\"platform\":\"pc\",
\"sdkuid\":\"<sdkuid>\",\"sessionid\":\"1-<base64>\",
\"sdk_version\":\"4.10.0\",\"udid\":\"<32hex>\",
\"deviceid\":\"<deviceid>\",\"aim_info\":\"{...}\",
\"client_login_sn\":\"<32hex>\",\"gas_token\":\"\",
\"source_platform\":\"pc\",\"ip\":\"<客户端IP>\",
\"get_access_token\":\"1\",\"access_token\":\"<JWT>\"}"}
```

`sessionid` 解码（`1-` 前缀 + base64）：

```json
{"prs":30,"lt":1793625548,"odi":"<deviceid>",
 "s":"[mp]-<32字符>-<64hex>",
 "u":"<sdkuid>","t":1,"w":"t",
 "si":"<40hex>","g_i":"<gameid>","sa":3}
```

> `s` 字段是**服务端签发**的签名，不是本地生成——所以不需要逆向签名算法。
> 见 `NtUniSdkBase.dll` 中 `UNISDK_LOGIN_JSON extra.channel_raw_info → SESSION → sessionid`。

### 6.3 login-otp 响应（已验证）

```json
{"code":0,"message":"正常返回",
 "entity":{"otp":0,"otp_token":"<16位>","aid":<数字>,"lock_time":-1,"open_otp":1}}
```

### 6.4 authentication-otp（未攻克）

- 请求体 2657 字节，**高熵密文**（256 种字节均匀分布，开头 `kC0KHFCtlB0GMye1` 类 base64 头）
- 响应体 2520 字节 gzip，解压后仍是**加密二进制**
- 推测：RSA 协商 + AES 会话密钥，密钥逻辑在 `NtUniSdkBase.dll` / `NtUniSdkRoostX.dll`

**下一步方向**：
1. 用 IDA 分析 `NtUniSdkRoostX.dll`（HTTP 库）的加密初始化
2. hook 熵源/随机数生成，观察会话密钥何时产生
3. 对比两次抓包的密文，判断是"每次随机 IV"还是"固定密钥"

## 7. 已知限制

1. **同账号互斥**：挂房期间该账号不能再登录游戏/启动器（`code:22`）
2. **token 生命周期**：与启动器会话绑定，启动器退出即注销（不是空闲过期——每 5 秒发请求也续不住）
3. **access_token 有效期**：JWT `exp` 约 3 天，`refresh_token` 机制存在但刷新接口是动态拼接的，未抓到
4. **合规风险**：模拟客户端协议可能违反用户协议，存在封号风险

## 8. 工具与产物

```
crates/lobby-core        协议 + 配置 + 保活状态机
crates/lobby-daemon      服务器常驻进程
docs/protocol.md         协议速查
docs/reverse-engineering.md  本文档
re/FeverGamesLauncher.exe.i64   IDA 数据库（未公开，含目标二进制分析）
```

### 8.1 IDA 使用备忘

- `idat64.exe`（文本版）在 Python 3.14 环境下启动即失败（`No module named 'imp'`）
- 用 **`ida64.exe -B -A <file>`**（GUI 版的批处理模式）可以正常生成 `.asm` / `.i64`
- 需要先用 `idapyswitch.exe` 把 Python 切到 3.9 或更老的版本

## 9. 时间线

| 阶段 | 结论 |
|---|---|
| 协议侦察 | 建房/心跳全部明文，无签名 |
| 认证尝试 | 发现 token 与会话绑定，`code:22` 互斥 |
| 登录抓包 | 系统代理/WinHTTP/CEF 全部失败 |
| IPC 逆向 | 定位 `WM_COPYDATA` + 窗口类名，加密未解 |
| 注册表发现 | **登录态明文持久化** |
| login-otp | **明文协议，可重放** |
| authentication-otp | 加密墙，待攻克 |

## 10. 致谢与免责

本文档仅用于协议学习与研究。请勿用于违反服务条款的用途。
所有账号、token、设备 ID 均已脱敏。
