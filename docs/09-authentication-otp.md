# 09 登录第二步 authentication-otp

> **这是目前唯一没攻克的环节。** 这篇记录我们已知的所有信息，方便后来者接手。

## 位置

```text
Host: x19obtcore.nie.netease.com:8443
POST /authentication-otp
```

## 已知特征

| 项 | 观测值 |
|---|---|
| 请求体长度 | 2657 字节 |
| 请求体性质 | 高熵密文，256 种字节均匀分布 |
| 请求体开头 | `kC0KHFCtlB0GMye1`（类 base64 头，可能是 IV/盐） |
| 响应头 | `Content-Encoding: gzip`，`Content-Length: 2520` |
| 响应体性质 | gzip 解压后**仍是加密二进制** |
| 耗时 | 约 100 ms |

## 我们做过的事

### 尝试一：按明文解析（失败）

请求体里找不到任何 JSON 结构，也没有可读字符串。

### 尝试二：找固定前缀（失败）

两次抓包的密文前缀不同，说明**不是固定密钥直接 XOR**。

### 尝试三：滚动 XOR 爆破（失败）

在 `FeverGamesLauncher.exe` 里发现过一段 XOR 循环（使用 496 字节密钥表），
但在 `authentication-otp` 的数据上试了 **1600 种起始偏移组合**，
可读率最高只有 0.43，且那部分本来就是明文头。

> 结论：那张表**不是** `authentication-otp` 的密钥。

### 尝试四：找公开资料（失败）

搜索 `unisdk authentication-otp` 等关键词，未找到任何公开逆向成果。
唯一相关的[网易 MC Api 汇总](https://www.cnblogs.com/mmme/p/17023909.html)只列了服务端地址。

## 当前推断

```text
authentication-otp
  ├─ 请求：加密( otp_token + 设备信息 + 时间戳 + ... )
  └─ 响应：加密( 会话信息，含 16 位 user-token )
```

加密方案的两个候选：

| 候选 | 依据 | 验证方法 |
|---|---|---|
| RSA 协商 + AES 会话密钥 | 开头有类 base64 头，长度 2657 不是 AES 块大小的整数倍（可能含头部） | hook 随机数生成，看是否每次不同 |
| 服务端公钥 RSA 加密整包 | 响应长度 2520 与请求 2657 量级相近 | 找客户端内置的 RSA 公钥 |

## 三个可执行的下一步

### 方向一：hook 熵源

观察 `authentication-otp` 加密前的随机数何时产生：

```javascript
// 候选 hook 点
Module.getExportByName("crypt32.dll", "CryptGenRandom");
Module.getExportByName("bcrypt.dll", "BCryptGenRandom");
Module.getExportByName("advapi32.dll", "SystemFunction036");  // RtlGenRandom
```

如果每次加密前都有一次随机数生成 → 会话密钥是随机的 → 需要抓完整握手。

### 方向二：hook AES / RSA 原语

```javascript
// OpenSSL 1.1（若 SDK 内置）
Module.getExportByName("libcrypto-1_1.dll", "EVP_EncryptInit_ex");
Module.getExportByName("libcrypto-1_1.dll", "AES_cbc_encrypt");

// Windows CNG
Module.getExportByName("bcrypt.dll", "BCryptEncrypt");
```

在 `BCryptEncrypt` / `EVP_EncryptInit_ex` 上打日志，能直接看到**明文参数与密钥句柄**。

### 方向三：对比两次抓包

同一账号连续两次登录，比较密文：

| 观察 | 结论 |
|---|---|
| 完全相同 | 固定密钥 + 固定 IV（可能可以直接复用） |
| 仅部分字节相同 | 固定密钥 + 随机 IV |
| 完全不同 | 每次会话密钥协商 |

## 已知的上下文（能省时间）

1. 加密逻辑在 `NtUniSdkBase.dll`（4.4 MB）或 `NtUniSdkRoostX.dll`（7.7 MB）
2. 相关字符串线索：

```text
UNISDK_LOGIN_JSON oauth2.access_token=%s, oauth2.refresh_token=%s
UNISDK_JF_REFRESH_TOKEN
refresh_token
```

3. `login-otp` 返回的 `otp_token`（16 位）一定参与 `authentication-otp` 的加密
4. IDA 数据库已生成在本地（未公开，因含目标二进制分析）

## 如果你要接手

建议顺序：

1. 先读 [13 踩坑清单](13-pitfalls.md)（能省几小时）
2. 用 [04 Schannel Hook](04-schannel-hook.md) 复现抓包
3. 从方向一（hook 熵源）开始，成本最低
4. 如果发现是固定密钥，直接跳到方向三

## 相关

- 上一步：[08 登录第一步](08-login-otp.md)
- 本地凭证：[10 本地凭证存储](10-local-credentials.md)
