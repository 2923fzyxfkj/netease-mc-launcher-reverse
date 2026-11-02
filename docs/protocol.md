# 联机大厅协议速查

> 本文件是可直接照着实现的协议摘要。
> 逆向过程、抓包方法与踩坑记录见 [`reverse-engineering.md`](reverse-engineering.md)。

## 网关

| 用途 | 地址 |
|---|---|
| 业务 API | `https://x19apigatewayobt.nie.netease.com` |
| 登录/会话 | `https://x19obtcore.nie.netease.com:8443` |

客户端会绕过系统代理直连（其配置中 `defaultProxy enabled="false"`）。

## 通用请求头

```
X_TRACE_ID: <32 位字母数字随机串，每次请求都变>
user-id:    <数字账号 ID>
user-token: <16 位会话 token，末位恒为 '1'>
User-Agent: WPFLauncher/1.15.31.20647
```

### 致命陷阱

**不要发送 `Content-Type`。**

实测：若被客户端库自动补上 `application/x-www-form-urlencoded`，
服务端会按表单解析，导致 body 全空，返回 `code:4 参数为空` 或 `code:13 操作失败`。

Python 的 `urllib` 会自动补这个头，请改用 `http.client` 或显式清空。

## 1. 建房

```http
POST /online-lobby-room
```

```json
{"room_name":"...","max_count":2,"visibility":0,"res_id":"<地图ID>","save_id":"","password":""}
```

- `visibility`：`0` = 公开
- `password`：空字符串 = 无密码
- `res_id`：作品/地图 ID；**地图未下载时建房会失败**

## 2. 心跳保活

```http
POST /online-lobby-member-tick     # 每 30 秒
```

```json
{"room_id":"<房间ID>","password":null,"check_visibilily":false}
```

- 字段名就是 `check_visibilily`（官方拼写错误），**不要修正**
- 无密码时 `password` 传 `null`
- 成功响应：

```json
{"code":0,"message":"正常返回","details":"","entity":null}
```

## 3. 登录（未完全攻克）

### 3.1 第一步：login-otp（✅ 明文，已验证可重放）

```http
POST /login-otp
Host: x19obtcore.nie.netease.com:8443
```

请求体为明文 JSON：

```json
{"sauth_json":"{\"gameid\":\"x19\",\"login_channel\":\"netease\",\"app_channel\":\"a50_sdk_cn\",\"platform\":\"pc\",\"sdkuid\":\"...\",\"sessionid\":\"1-...\",\"sdk_version\":\"4.10.0\",\"udid\":\"...\",\"deviceid\":\"...\",\"aim_info\":\"{...}\",\"client_login_sn\":\"...\",\"gas_token\":\"\",\"source_platform\":\"pc\",\"ip\":\"...\",\"get_access_token\":\"1\",\"access_token\":\"...\"}"}
```

成功响应：

```json
{"code":0,"message":"正常返回","entity":{"otp":0,"otp_token":"<16位>","aid":<数字>,"lock_time":-1,"open_otp":1}}
```

### 3.2 第二步：authentication-otp（❌ 端到端加密）

```http
POST /authentication-otp
```

- 请求体：约 2657 字节的高熵密文
- 响应体：约 2520 字节，gzip 解压后仍是加密二进制
- 这一步换取最终的 16 位 `user-token`

**尚未攻克**，需要逆向 `NtUniSdkBase.dll` / `NtUniSdkRoostX.dll` 的密钥协商。

### 3.3 第三步：login-start

```http
POST /interconn/web/game-play-v2/login-start
Host: x19obtcore.nie.netease.com:8443
user-token: <16位>
```

返回 `{"code":0,...}`，表示游戏会话已建立。

## 4. 错误码

| code | 含义 | 常见原因 |
|---|---|---|
| 0 | 成功 | |
| 4 | 参数为空 | body 未被解析（Content-Type 陷阱） |
| 10 | 请先登录 | token 随客户端会话失效 |
| 12 | 参数错误 | token 无效/伪造 |
| 13 | 操作失败 | 高频重复调用 |
| 22 | 您的帐号在另一处登录 | 同账号互斥 |
