# TUI / Host 与手机体验修复记录

## 现场证据

- 运行中的 TUI 来自 `output/releases/tui-20261028-211829/ExpertChat-TUI`。
- 发现文件指向 PID 50748、端口 52289；检查时进程已退出，端口无监听。
- Windows Application 1000 事件在 2026-10-29 01:56:52、02:01:36 记录了
  `expert-host.exe` 的相同异常 `0xc0000409`，模块偏移 `0x32e2ed`。
- 本机两份转储的原始异常为 `0xc0000005`。使用对应发行版 EXE 内的 ELF
  符号和保存的帧链定位，两次均经过：

  ```text
  TaskBudget.answerStarted → _syncClock → 检查点 → WorkRuntimeLogs.add
  → retainRuntimeLogs → JsonCodec.encode → StringBuffer._addPart
  ```

未读取或导出对话正文、模型请求或凭据。函数名和偏移记录在
`output/host-stability/crash-frames.json`。

该证据确定了两次崩溃的位置；未能在合成压力测试中重现底层内存异常，
不能据此宣称已经证明具体的 Dart VM 或 FFI 缺陷。

## 修改

- 日志记录的 JSON 和字节数每条缓存一次；使用字节编码器规避上述
  StringBuffer 编码路径。保存快照在合并定时器或 flush 时生成。
- TUI 保留启动版本检查；首次连接失败后持续重试；Host 退出后重新发现并
  按既有 auto-start 配置拉起。恢复过程不重放变更命令。
- TUI 将普通断线、认证拒绝与协议不兼容分开显示，不再统一要求结束旧后台。
- GUI 增加 WebSocket 活性检测及单个有超时的 RPC 心跳；断开后允许重新启动
  Host；忽略旧连接迟到的数据。
- 手机增加底部导航，键盘弹出时收起；页面首次访问才创建，隐藏页暂停 ticker。
- 手机输入字号 16，发送/停止/语音按钮触控区域 44；手机 Enter 保留换行行为；
  拖动消息列表收起键盘，后台会话更新不再触发当前消息列表的 provider 重建。

## 验证

- 修改涉及的 11 个 Dart 文件静态检查通过。
- 手机导航/输入/响应布局/懒加载/桌面布局/GUI 连接恢复回归：29 项通过。
- 日志/桥接协议/窗口生命周期回归：36 项通过。
- Rust transport：10 项通过；请求 ID 与不重放单元测试通过；release 构建通过。
- 日志原生 AOT 压力测试：同样 10,000 次插入、12 会话、混合 Unicode/转义字符，
  保留 912 条；原路径 89,066 ms，新路径约 3,300 ms。该结果是日志处理吞吐，
  不是手机帧率测量。
- 完整 AOT Host 独立进程验收通过：双客户端、资料锁、历史持久化、崩溃恢复、
  拒绝重放旧操作；仅使用本地合成模型。
- 真实 Rust TUI 使用隔离 profile 自动启动 Host；测试强制结束该 Host 后，
  TUI 自动拉起新 PID 并恢复快照。报告 `output/host-stability/tui-recovery.json`。
- 8 个合成任务、每秒 40 个事件、90 秒活动负载及后续空闲测量通过；
  TUI 事件交付 P95 275 ms，GUI 214 ms，Host 空闲单核占用约 0.23%。
  这些是协议交付指标，不包含原生绘制。报告 `output/host-stability/host-load.json`。

没有关闭生产 TUI、覆盖运行中的文件或重放真实任务。未进行手机真机帧率、
输入法或长时间用户负载验收。覆盖补丁只包含 TUI 与 Host，手机源码改动需重新构建。

## 使用修复包

`output/host-stability/TUI-Host-Hotfix.zip` 是覆盖补丁。先正常关闭客户端，
再把其中的 `expert-tui.exe` 和完整 `host` 目录复制到原完整发行目录。
详见包内 README。原数据目录不属于补丁。
