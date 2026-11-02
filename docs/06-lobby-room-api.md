# 06 建房

> 联机大厅创建房间的接口。**纯 HTTP，无签名，可直接复现。**

## 请求

```http
POST /online-lobby-room HTTP/1.1
Host: x19apigatewayobt.nie.netease.com
X_TRACE_ID: <32 位字母数字随机串>
user-id: <数字账号 ID>
user-token: <16 位会话 token>
User-Agent: WPFLauncher/1.15.31.20647
Accept-Encoding: gzip, deflate
Content-Length: <body 长度>

{"room_name":"...","max_count":2,"visibility":0,"res_id":"<地图 ID>","save_id":"","password":""}
```

## 字段说明

| 字段 | 类型 | 说明 |
|---|---|---|
| `room_name` | string | 房间名 |
| `max_count` | int | 人数上限 |
| `visibility` | int | 公开性，实测 `0` = 公开 |
| `res_id` | string | 作品/地图 ID，**地图未下载时建房会失败** |
| `save_id` | string | 存档 ID，可为空 |
| `password` | string | 空字符串表示无密码 |

## 请求头说明

| 头 | 说明 |
|---|---|
| `X_TRACE_ID` | 32 位随机字母数字，每次请求都变，无校验作用 |
| `user-id` | 数字账号 ID |
| `user-token` | 16 位会话 token，**末位恒为 `1`** |

## 致命陷阱：Content-Type

> [!WARNING]
> **绝对不能带 `Content-Type: application/x-www-form-urlencoded`。**
>
> Python 的 `urllib` 在 POST 时会自动补这个头，服务端按表单解析，body 全空，
> 返回 `code:4 参数为空` 或 `code:13 操作失败`。
>
> 这个坑浪费了很长时间，因为从错误码完全看不出根因。

规避方式：

```python
# ❌ urllib 会自动加 Content-Type: application/x-www-form-urlencoded
urllib.request.urlopen(req)

# ✅ 用 http.client 手动控制
conn = http.client.HTTPSConnection(HOST, 443)
conn.request("POST", "/online-lobby-room", body=payload, headers=headers)
```

如果一定要用 `urllib`，显式设置 `Content-Type: application/json` 也能通过
（实测两种都返回 `code:0`），但**保持与客户端一致（不发该头）最稳**。

## 成功响应

```json
{"code":0,"message":"正常返回","details":"","entity":{...}}
```

`entity` 中的 `room_id` 是后续心跳要用的凭据。字段名可能是 `room_id` / `roomId` / `id`，
建议实现时同时兼容。

## 完整可运行示例

见 [`examples/01-create-room`](../examples/01-create-room/src/main.rs)。

## 错误码

| code | 含义 | 常见原因 |
|---|---|---|
| 0 | 成功 | |
| 4 | 参数为空 | body 未被解析（Content-Type 陷阱） |
| 10 | 请先登录 | token 随客户端会话失效 |
| 12 | 参数错误 | token 无效 |
| 22 | 您的帐号在另一处登录 | 客户端正在运行，同账号互斥 |

## 前置条件

1. **地图已下载**：`res_id` 对应的作品必须已安装在客户端
2. **token 有效**：客户端登录会话存活
3. **客户端未运行**：同账号互斥（想纯协议建房必须先关客户端，但关了 token 又失效——这正是卡点）

## 下一步

- 房间建好了，怎么保持在线 → [07 心跳保活](07-lobby-heartbeat.md)
