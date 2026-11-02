# 网易我的世界登录器逆向笔记

> 一份关于网易《我的世界》中国版电脑端的完整逆向笔记：从 `sessionId`、`user-token` 到联机大厅 API。
> 全程由 AI Agent（DeepSeek + DeepSeek Harness）探索，人类只负责按「允许」。

[![License](https://img.shields.io/badge/license-MIT-blue)](LICENSE.md)

## 这是什么

这是一个**纯文档仓库**，不是可运行的作弊工具。它记录了一次完整的协议逆向过程，包括：

- 为什么常规抓包对网易客户端全部失效，以及怎么绕过
- 联机大厅的建房与心跳协议（已完全破解，纯 HTTP）
- 登录流程的三步链路（第一步明文可重放，第二步加密未攻克）
- 登录态在注册表里的明文存储位置
- 本地 IPC（ticket 传递）的通道与消息格式

**给谁看**：对网易客户端协议、Windows 抓包、Frida 注入、Schannel 逆向感兴趣的研究者。

## 结论速览

| 环节 | 状态 | 说明 |
|---|---|---|
| 建房 `POST /online-lobby-room` | 已破解 | 纯 HTTP，可复现 |
| 心跳 `POST /online-lobby-member-tick` | 已破解 | 30 秒周期，可复现 |
| 会话凭证 `user-token` | 受限 | 随客户端会话失效，关掉启动器即 `code:10` |
| 登录第一步 `POST /login-otp` | 已破解 | 明文 JSON，已验证可重放 |
| 登录第二步 `POST /authentication-otp` | 未攻克 | 请求与响应均为端到端加密 |
| 本地登录态 | 已定位 | 明文持久化在注册表 |
| 本地 IPC（ticket） | 已定位 | `WM_COPYDATA` + `LHMW_FG_Main`，数据加密未解 |

## 目录

### 基础

- [01 背景与目标](docs/01-overview.md) — 想解决什么问题、最后做到哪一步
- [02 客户端拓扑](docs/02-client-topology.md) — 三个进程、两套 TLS、一张图说清

### 抓包方法论

- [03 抓包环境与选型](docs/03-capture-setup.md) — 为什么代理方案全废，该用什么
- [04 Schannel Hook](docs/04-schannel-hook.md) — hook `secur32` 拿明文，含完整 Frida 脚本
- [05 进程启动瞬间注入](docs/05-process-injection.md) — 抓「启动第一毫秒」的请求

### 联机大厅协议

- [06 建房](docs/06-lobby-room-api.md) — `POST /online-lobby-room`
- [07 心跳保活](docs/07-lobby-heartbeat.md) — `POST /online-lobby-member-tick`

### 登录与凭证

- [08 登录第一步 login-otp](docs/08-login-otp.md) — 明文协议，已验证可重放
- [09 登录第二步 authentication-otp](docs/09-authentication-otp.md) — 加密墙，尚未攻克
- [10 本地凭证存储](docs/10-local-credentials.md) — 注册表明文登录态
- [11 本地 IPC 与 ticket](docs/11-ipc-ticket.md) — `WM_COPYDATA` 通道

### 工程与经验

- [12 IDA 工作流备忘](docs/12-ida-workflow.md) — 版本、参数、踩过的坑
- [13 踩坑清单](docs/13-pitfalls.md) — 25 个具体问题与复现方式
- [14 事件时间线](docs/14-timeline.md) — 从 0 到「只差一道墙」

### 示例代码

- [Rust 示例说明](examples/README.md) — 三个可编译的最小示例

## 仓库结构

```text
README.md                    本文件（目录与简介）
docs/                        按环节拆分的文档
  01-overview.md
  02-client-topology.md
  03-capture-setup.md
  04-schannel-hook.md
  05-process-injection.md
  06-lobby-room-api.md
  07-lobby-heartbeat.md
  08-login-otp.md
  09-authentication-otp.md
  10-local-credentials.md
  11-ipc-ticket.md
  12-ida-workflow.md
  13-pitfalls.md
  14-timeline.md
examples/
  README.md
  01-create-room/            Rust：只建一次房
  02-heartbeat/              Rust：建房 + 心跳保活
  03-token-probe/            Rust：token 有效性探针
LICENSE.md                   MIT
```

## 已知限制

- **同账号互斥**：同一账号同一时间只能一处在线，并发会 `code:22`。
- **token 与会话绑定**：客户端退出即注销，实测每 5 秒发请求也续不住。
- **第二步加密未解**：`authentication-otp` 请求与响应均为高熵密文。
- **合规风险**：模拟客户端协议可能违反用户协议，仅供学习研究。

## 贡献

欢迎补充 `authentication-otp` 的加密分析。动手前请先读 [13 踩坑清单](docs/13-pitfalls.md)，能省掉大量重复劳动。

## 许可

[MIT](LICENSE.md)

---

本项目为协议学习与研究用途。所有账号、token、设备标识均已脱敏。
