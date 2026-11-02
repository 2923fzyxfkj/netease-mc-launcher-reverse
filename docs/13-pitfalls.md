# 13 踩坑清单

> 25 个具体问题，每个都带现象与复现方式。**动手前先看这篇，能省几小时。**

## 抓包类

### 1. 系统代理抓不到启动器的包

**现象**：mitmproxy 设了系统代理，只抓到 CEF 页面请求，没有任何 API 请求。

**原因**：`WPFLauncher.exe.config` 里 `defaultProxy enabled="false"`。

**对策**：放弃代理，改 hook `secur32`。

### 2. WinHTTP 代理是另一套配置

**现象**：改了系统代理仍然抓不到。

**原因**：WinHTTP 与 WinINet 是两套独立代理配置。

**对策**：

```powershell
netsh winhttp set proxy <host>:<port>   # 只影响 WinHTTP
```

但仍不保证能抓到——最终还是要 hook Schannel。

### 3. CEF 不读系统证书库

**现象**：把 CA 装进「受信任的根证书颁发机构」后，CEF 请求仍报证书错误。

**原因**：Chromium 用自带的 BoringSSL 与证书校验逻辑。

**对策**：CEF 这条路基本放弃，抓原生进程。

### 4. Wireshark 抓到 0 个包

**现象**：`tshark -i <接口>` 显示 0 packets。

**原因**：选错了网卡（有线口没流量，实际走无线）。

**对策**：

```powershell
ipconfig    # 找到真正有 IP 的网卡
```

### 5. 代理软件的 TUN 模式干扰

**现象**：抓到的全是本地回环流量。

**原因**：Clash / V2Ray 的 TUN 模式把流量劫持到虚拟网卡。

**对策**：抓包期间临时退出代理软件。

## Frida 类

### 6. `hexdump` 变量名冲突

**现象**：`TypeError: cannot define variable 'hexdump'`。

**原因**：Frida 运行时已内置 `hexdump`。

**对策**：改用 `toHex` / `toHexStr` 之类的自定义名。

### 7. `enc=false dec=false`

**现象**：hook 装上了，但两个导出都是 `false`。

**原因**：注入太早，`.NET` 还没加载 `secur32.dll`。

**对策**：轮询等待模块出现再挂 hook。

### 8. `access violation accessing 0x4`

**现象**：`DecryptMessage` 的 `onLeave` 里读 buffer 直接崩。

**原因**：**`DecryptMessage` 的 `pMessage` 是第 2 个参数（`a[1]`）**，
不是第 3 个（`a[2]`）。`a[2]` 是 `MessageSeqNo`，一个很小的整数。

**对策**：`SendMessage` 与 `DecryptMessage` 分别用 `a[2]` / `a[1]`。

### 9. `frida.attach` 死锁

**现象**：脚本卡住，没有任何输出。

**原因**：在 Frida 的 Python 回调线程里调用 `frida.attach()`。

**对策**：回调只入队，主线程负责 `attach`。

### 10. attach 挂起进程死锁

**现象**：给子进程加 `CREATE_SUSPENDED` 后 `attach` 永不返回。

**原因**：挂起的进程主线程不运行，`DllMain` 不执行。

**对策**：不要 attach 挂起的进程；改成「正常启动 + 快速注入」。

### 11. spawn 模式不触发登录

**现象**：用 `device.spawn()` 启动 WPFLauncher，hook 正常但没有任何登录请求。

**原因**：绕过父进程（发烧游戏），缺少触发登录的上下文。

**对策**：必须走正常启动流程。

## 协议类

### 12. `code:4 参数为空`

**现象**：明明发了 body，服务端说参数为空。

**原因**：**Python `urllib` 自动补了 `Content-Type: application/x-www-form-urlencoded`**，
服务端按表单解析。

**对策**：用 `http.client` 手动控制请求头，或用 `reqwest` 的 `.body(bytes)` 代替 `.json()`。

### 13. `code:13 操作失败`

**现象**：心跳请求返回操作失败。

**原因**：
- 同一时刻重复心跳（客户端刚发过，你又发一次）
- 或者同第 12 条（Content-Type 陷阱）

**对策**：保持 30 秒周期，不要并发。

### 14. `code:22 另一处登录`

**现象**：客户端运行时，工具建房返回另一处登录。

**原因**：同账号互斥。

**对策**：需要客户端完全退出，但这又会导致 token 失效——这是本项目的核心矛盾。

### 15. `code:10 请先登录`

**现象**：客户端退出后，工具用旧 token 请求返回请先登录。

**原因**：token 随会话注销（不是空闲过期）。

**验证**：每 5 秒发一次请求，客户端一退出 5 秒内就变 `code:10`——**续不住**。

### 16. 字段名 `check_visibilily`

**现象**：把字段「修正」成 `check_visibility` 后行为异常。

**原因**：官方拼写就是少一个 `i`。

**对策**：原样保留。

### 17. 建房失败：地图未下载

**现象**：`POST /online-lobby-room` 失败。

**原因**：`res_id` 对应的作品还没下载完成。

**对策**：先在客户端里把地图下载好。

## 逆向类

### 18. `idat64` 启动即挂

**现象**：`No module named 'imp'`。

**原因**：IDA 7.6 的 IDAPython 不兼容 Python 3.12+。

**对策**：`idapyswitch -s <Python39>/python3.dll`。

### 19. `idat64` 静默退出

**现象**：退出码 1，无产物、无日志。

**对策**：改用 `ida64.exe -B -A <file>`（GUI 版的批处理模式）。

### 20. 内存扫描把客户端搞崩

**现象**：用 Frida `Memory.scanSync` 扫大范围内存后，启动器崩溃。

**原因**：大范围扫描触发了它的保护机制。

**对策**：不要扫内存；用 hook 拿数据。

### 21. `Get-DnsClientCache` / `Get-NetTCPConnection` 拒绝访问

**现象**：这两个命令报「拒绝访问」。

**原因**：权限不足（普通用户）。

**对策**：用 `ipconfig /displaydns` 和 `netstat -ano` 替代。

### 22. IP 路径含方括号导致工具失败

**现象**：`Get-Item` 报找不到文件，但文件明明存在。

**原因**：路径里的 `[` `]` 被当成通配符。

**对策**：所有命令加 `-LiteralPath`。

## 环境类

### 23. `reg query /s` 超时

**现象**：全盘搜注册表时命令超时。

**对策**：先定位顶层键（`Get-ChildItem 'HKCU:\Software'`），再缩小范围查。

### 24. `curl` 返回 000

**现象**：`curl` 访问任何 HTTPS 都返回 `000`，但 Python 能正常访问。

**原因**：curl 用 Schannel，在受限环境里凭证初始化失败。

**对策**：改用 Python 的 `urllib` / `requests` 做请求。

### 25. 中文路径与编码

**现象**：PowerShell 输出中文乱码，Python 打印乱码。

**对策**：

- PowerShell：设置 `[Console]::OutputEncoding = [Text.Encoding]::UTF8`
- Python：写文件而不是打印，用 `errors="replace"` 兜底

## 一句话总结

> **这个项目的 80% 时间花在「抓不到」和「以为抓到了其实是误匹配」上。**
> 先把抓包链路验证正确，再谈协议。
