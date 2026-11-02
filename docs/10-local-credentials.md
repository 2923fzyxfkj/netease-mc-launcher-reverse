# 10 本地凭证存储

> **整个项目最重要的一次发现**：登录态是明文持久化的，不需要启动器运行就能读。

## 位置

```text
HKCU\Software\FeverGames\FeverGamesInstaller\data\token
```

值类型：`REG_BINARY`，内容是 **Base64 编码的 JSON**。

## 解码后的结构

```json
{
  "account_id": "<数字>",
  "account_name": "<邮箱>",
  "age_info": { "birthday": 0, "iso_code": "", "minor_status": 0, "pin_authorized_actions": [], "sign": "" },
  "avatar": "https://...",
  "gender": 0,
  "name": "<昵称>",
  "newUser": false,
  "token": "<64 位>",
  "type": 10,
  "userId": "<8 位 ID>"
}
```

| 字段 | 说明 |
|---|---|
| `account_id` | 网易账号数字 ID |
| `userId` | 发烧游戏内部 ID（8 位） |
| `token` | 64 位字符串，登录凭证的一部分 |
| `type` | 固定 `10` |

## 同目录的其他值

| 值名 | 内容 |
|---|---|
| `data\sdkuid` | 16 位用户标识（`sauth_json` 里要用） |
| `data\settingSocial` | 社交可见性设置（Base64 JSON） |
| `setting\abtest` | 含 `device_id`、`login_id` |
| `game\<gid>\StartupParams` | 启动参数（如 `--start_from_launcher=1`） |

## 为什么这个发现关键

它把「无图形环境」从不可能变成了「有希望」：

```text
之前：登录态只在内存里 → 必须有客户端跑着 → 必须有图形桌面
之后：登录态明文在注册表 → 关掉客户端也能读 → 服务器上可以直接拿
```

## 读取方式

Python：

```python
import subprocess, base64, json, re

def read_reg_binary(path, name):
    out = subprocess.run(
        ["reg", "query", path, "/v", name],
        capture_output=True, text=True, errors="replace"
    ).stdout
    m = re.search(r"REG_BINARY\s+([0-9A-Fa-f]+)", out)
    if not m:
        return None
    raw = bytes.fromhex(m.group(1))
    # 实际内容可能是 Base64 文本，也可能是纯二进制
    try:
        return base64.b64decode(raw).decode("utf-8")
    except Exception:
        return raw.decode("utf-8", "replace")

PATH = r"HKCU\Software\FeverGames\FeverGamesInstaller\data"
text = read_reg_binary(PATH, "token")
data = json.loads(text)
print(data["userId"], data["account_id"])
```

PowerShell：

```powershell
$raw = (Get-ItemProperty 'HKCU:\Software\FeverGames\FeverGamesInstaller\data').token
$text = [Text.Encoding]::UTF8.GetString($raw)
if ($text -match '^[A-Za-z0-9+/=]+$') {
    $text = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($text))
}
$text | ConvertFrom-Json
```

## 但它不能直接当 user-token

注册表里的 `token` 是 **64 位**，而联机大厅 API 要的是 **16 位** `user-token`（末位恒为 `1`）。

两者的关系还没完全打通：

```text
注册表 token (64 位)
      ↓ ?
access_token (JWT)
      ↓ UniSDK 登录
sessionid
      ↓ login-otp → authentication-otp
user-token (16 位)
```

详见 [08 登录第一步](08-login-otp.md)。

## 其他可能的凭证位置

| 位置 | 内容 |
|---|---|
| `%LOCALAPPDATA%\Netease\MCLauncher\config\user\<账号>\` | MC 启动器的本地配置（`.config` / `.db`，加密） |
| `%LOCALAPPDATA%\FeverGames\FeverGamesInstaller\cache\CefCache` | CEF 的 Cookie / LocalStorage |
| `C:\MCLDownload\Game\<账号>\` | 世界存档 |

> [!NOTE]
> MC 启动器的配置是 `!x19sign!` 加密的，日志也一样。但**发烧游戏这边的注册表是明文**——
> 这是两条完全不同的存储链路，别混为一谈。

## 免责

本文档仅记录存储位置与格式，不包含任何真实账号信息。请勿用于未授权访问。
