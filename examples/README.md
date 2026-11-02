# Rust 示例

> 三个独立可编译的最小示例，分别演示：建一次房、建房+心跳保活、token 有效性探针。

## 前提

| 项 | 要求 |
|---|---|
| Rust | 1.75+ |
| 凭证 | 一个有效的 `user-id` 与 `user-token` |

获取凭证的方式见 [10 本地凭证存储](../docs/10-local-credentials.md)（注意：注册表里的是 64 位 token，与 API 要的 16 位 `user-token` **不是同一个**）。

## 示例一览

| 目录 | 作用 | 关键点 |
|---|---|---|
| `01-create-room` | 只调一次建房接口 | 不带 `Content-Type` |
| `02-heartbeat` | 建房后每 30 秒心跳，连续失败则重建 | 状态机 + 退避 |
| `03-token-probe` | 判断 token 是否还有效 | 区分 `code:10` / `code:22` / `code:0` |

## 运行方式

每个示例是独立的 crate：

```bash
cd 01-create-room
cargo run -- --user-id <你的 user-id> --token <你的 user-token> --room-name 测试房间 --res-id <地图 ID>
```

## 三个共性坑（示例里已处理）

1. **不要发 `Content-Type`**：`reqwest` 的 `.json()` 会自动补 `application/json`；
   本仓库示例用 `.body(bytes)` 保持与客户端一致。
2. **字段名是 `check_visibilily`**：少一个 `i`，不要「修正」。
3. **直连不走系统代理**：`.no_proxy()`，与客户端行为一致。

## 编译检查

```bash
cd 01-create-room && cargo check
cd ../02-heartbeat && cargo check
cd ../03-token-probe && cargo check
```

## 注意

这些示例只演示协议调用，**不包含登录逻辑**。
`user-token` 需要自行获取，且随客户端会话失效——详见 [01 背景与目标](../docs/01-overview.md)。
