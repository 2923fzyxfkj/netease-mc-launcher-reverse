# 08 登录第一步 login-otp

> **这是整条认证链里唯一明文的一环，已实测可重放。**

## 位置

```text
Host: x19obtcore.nie.netease.com:8443     ← 注意不是 x19apigatewayobt
POST /login-otp
```

## 请求体（明文 JSON）

```json
{
  "sauth_json": "{\"gameid\":\"x19\",\"login_channel\":\"netease\",\"app_channel\":\"a50_sdk_cn\",\"platform\":\"pc\",\"sdkuid\":\"<sdkuid>\",\"sessionid\":\"1-<base64>\",\"sdk_version\":\"4.10.0\",\"udid\":\"<32 hex>\",\"deviceid\":\"<deviceid>\",\"aim_info\":\"{...}\",\"client_login_sn\":\"<32 hex>\",\"gas_token\":\"\",\"source_platform\":\"pc\",\"ip\":\"<客户端 IP>\",\"get_access_token\":\"1\",\"access_token\":\"<JWT>\"}"
}
```

## sauth_json 里的字段

| 字段 | 说明 | 来源 |
|---|---|---|
| `gameid` | 固定 `x19` | 常量 |
| `login_channel` | 固定 `netease` | 常量 |
| `app_channel` | 固定 `a50_sdk_cn` | 常量 |
| `platform` / `source_platform` | 固定 `pc` | 常量 |
| `sdkuid` | 16 位用户标识 | 注册表明文 |
| `sessionid` | `1-` + base64(JSON) | UniSDK 登录响应 |
| `sdk_version` | 如 `4.10.0` | 客户端版本 |
| `udid` | 32 位 hex | 本机设备 ID |
| `deviceid` | 形如 `amawunqaaxvwfqyh-d` | 本机设备 ID |
| `client_login_sn` | 32 位 hex | 每次随机 |
| `aim_info` | 地区/ISP 信息 JSON | 客户端探测 |
| `ip` | 客户端公网 IP | 客户端上报 |
| `gas_token` | 空字符串 | — |
| `get_access_token` | `"1"` | 常量 |
| `access_token` | JWT | UniSDK 登录响应 |

## sessionid 的结构

去掉 `1-` 前缀后是 base64 的 JSON：

```json
{
  "prs": 30,
  "lt": 1793625548,
  "odi": "<deviceid>",
  "s": "[mp]-<32 字符>-<64 hex>",
  "u": "<sdkuid>",
  "t": 1,
  "w": "t",
  "si": "<40 hex>",
  "g_i": "<gameid>",
  "sa": 3
}
```

| 字段 | 说明 |
|---|---|
| `lt` | 登录时间戳（Unix 秒） |
| `odi` | 设备标识，与 `deviceid` 一致 |
| `u` | sdkuid |
| `si` | 会话 ID（40 位 hex） |
| `s` | **服务端签发的签名** |

> [!NOTE]
> `s` 字段（`[mp]-<32 字符>-<64 hex>`）**不是本地生成的**。
> 在 `NtUniSdkBase.dll` 里的来源是：
>
> ```text
> UNISDK_LOGIN_JSON extra.channel_raw_info → SESSION → sessionid
> ```
>
> 也就是说它是登录服务端下发的，客户端只做解析和缓存。
> **因此不需要逆向签名算法**——只要能拿到一份有效的登录响应即可。

## 响应（实测成功）

```json
{
  "code": 0,
  "message": "正常返回",
  "details": "",
  "entity": {
    "otp": 0,
    "otp_token": "<16 位>",
    "aid": 963747648,
    "lock_time": -1,
    "open_otp": 1
  }
}
```

| 字段 | 说明 |
|---|---|
| `otp` | `0` 表示无需二次验证 |
| `otp_token` | 下一步 `authentication-otp` 的凭据 |
| `aid` | 内部账号 ID |
| `lock_time` | `-1` 表示未锁定 |
| `open_otp` | `1` 表示 OTP 通道开启 |

## 已验证的事实

1. **接口是明文的**，无需任何加密即可构造
2. **可以重放**：用抓到的完整 `sauth_json` 重新 POST，返回 `code:0`
3. **不要求图形环境**：纯 HTTP
4. **返回的 `otp_token` 每次不同**，说明服务端正常签发

## 还没解决的

`sauth_json` 里有三个字段**我们无法凭空生成**：

- `sessionid`（含服务端签名 `s`）
- `access_token`（JWT，`exp` 约 3 天）
- `client_login_sn`（可随机，但可能参与校验）

其中 `sessionid` 与 `access_token` 都来自 UniSDK 的登录结果，
而那次登录发生在客户端启动早期，且**不是普通 HTTP**（抓不到）。

详见 [09 登录第二步](09-authentication-otp.md) 与 [10 本地凭证存储](10-local-credentials.md)。
