# 07 心跳保活

> 房间建好后，靠这个接口维持在线。**周期 30 秒，纯 HTTP。**

## 请求

```http
POST /online-lobby-member-tick HTTP/1.1
Host: x19apigatewayobt.nie.netease.com
X_TRACE_ID: <32 位字母数字随机串>
user-id: <数字账号 ID>
user-token: <16 位会话 token>
User-Agent: WPFLauncher/1.15.31.20647
Accept-Encoding: gzip, deflate
Content-Length: <body 长度>

{"room_id":"<房间 ID>","password":null,"check_visibilily":false}
```

## 字段说明

| 字段 | 类型 | 说明 |
|---|---|---|
| `room_id` | string | 建房返回的房间 ID |
| `password` | null | 无密码时为 `null` |
| `check_visibilily` | bool | **官方拼写错误**，不要修正 |

> [!WARNING]
> 字段名就是 `check_visibilily`（少一个 `i`）。
> 「修正」成 `check_visibility` 会导致服务端把它当未知字段，行为不可预期。

## 周期

实测为 **精确 30 秒**：

```text
11:04:33  tick
11:05:03  tick   (+30s)
11:05:33  tick   (+30s)
11:06:03  tick   (+30s)
11:06:33  tick   (+30s)
11:07:03  tick   (+30s)
```

同一批心跳的 `user-token` 保持不变（走同一条 keep-alive 连接）。

## 成功响应

```json
{"code":0,"message":"正常返回","details":"","entity":null}
```

## 失败处理

| 情况 | 建议 |
|---|---|
| 单次失败 | 记录日志，下次照常发 |
| 连续 N 次失败 | 判定掉线，重新建房 |
| `code:10` | token 已失效，无法自愈，需要重新获取凭证 |
| `code:22` | 账号在别处登录，检查是否有客户端在跑 |

## 实现要点

```rust
loop {
    tokio::time::sleep(Duration::from_secs(interval_secs)).await;

    match client.tick(&room_id).await {
        Ok(()) => failures = 0,
        Err(e) => {
            failures += 1;
            if failures >= max_failures {
                // 判定掉线，跳出重建
                break;
            }
        }
    }
}
```

两个容易忘的细节：

1. **重建前要等待**：房间刚掉线就立刻高频建房会触发服务端频控。
2. **房间 ID 会变**：重建后必须用新的 `room_id` 发心跳。

## 完整可运行示例

见 [`examples/02-heartbeat`](../examples/02-heartbeat/src/main.rs)。

## 下一步

- 想了解 token 怎么来的 → [08 登录第一步](08-login-otp.md)
- 踩坑清单 → [13 踩坑清单](13-pitfalls.md)
