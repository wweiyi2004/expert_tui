# 迁移完整性复核

2026-10-30 对照原工作区逐文件复核。结论范围是**独立 TUI 产品的源码、运行组件、资源与配套维护资料**。

首轮完成了可独立构建运行的迁移，但只保留了部分 Dart 回归测试，遗漏了辅助库、验收脚本与文档。本次已补齐并重新验证，不能用首轮的“构建成功”替代这次逐文件核对。

| 内容 | 来源文件数 | 复核结果 |
| --- | ---: | --- |
| Rust TUI（含全部 Rust 测试、examples、快照） | 114 | 无缺失；仅修改说明和独立存储/发现路径 |
| Rust 文件工具 | 8 | 无缺失；仅修改默认存储路径 |
| 终端设计、协议、夹具、宠物资源 | 30 | 无缺失，内容一致 |
| npm 启动器及更新器 | 8 | 无缺失；调整包身份、命令、缓存路径和说明 |
| `features/tui_bridge` | 27 | 22 个 Host/协议/终端文件已包含；5 个 Flutter GUI 适配文件明确排除 |
| `tui*test.dart` | 13 | 12 个移植为纯 Dart 活跃测试；1 个 GUI 插件集成测试原文归档 |

Host 的 Dart 依赖按 import/export/part 提取；固定为原生 Host 分支，包名改为 `expert_tui`。静态复核检查了 1716 个 Dart 导入及 Rust 编译资源引用，没有缺失的内部文件，也没有指向原项目的应用导入或 Flutter 包根。

## 已补齐的内容

- 独立的 `media_extract.dart` 辅助库及测试需要的工作区 stub。
- TUI 附件、桥接、凭据缓存、重试、快照证据、媒体配置、模型思考档位和工作台测试。通用假对象从原 Widget 测试中单独提取，不引入 Flutter。
- 协议夹具校验、Host 负载脚本、自动恢复脚本、子代理/会话导入端到端脚本和 Host 编译探针。
- 图片预览、Host 稳定性历史文档。

原测试的占位 PNG 被替换成有效的 1×1 PNG，使独立媒体 worker 能解码；图片内容、作用域 token、预览字节一致等断言保留。`python tool/test_runtime.py` 为整套测试提供本仓库媒体 worker 的绝对路径。

## 明确的排除与归档

没有迁入以下 5 个 GUI 适配文件：`bridge_host.dart`、`bridge_host_io.dart`、`bridge_host_stub.dart`、`gui_actions_flutter.dart`、`gui_navigation.dart`。它们属于 Flutter Widget、平台通道和页面导航，不承担独立 Host 的任务执行。

原 `tui_bridge_native_test.dart` 依赖 Flutter 的 preferences/path-provider 插件及其故障注入接口，已原文保存在 `archive/gui-integration/`，不计入独立 Host 测试通过数。

旧 GUI 构建脚本、旧发布说明、GUI 单实例测试、付费 live smoke 和界面 fixture 原文保存在 `archive/legacy-workflows/`，以 `.txt` 结尾。它们记录旧工程环境，不作为新项目可执行流程；独立构建使用 `tool/build_native.py` 和 `tool/build_tui_npm_release.py`。

GUI 页面、手机设备界面、原项目用户会话/密钥/配置不在迁移范围内。远程 MCP/网关服务仍是独立配置的外部服务；ffmpeg、可选数学环境等原有外部依赖也未变成内置二进制。

## 本次验证

- `dart analyze`：无问题。
- 纯 Dart 测试：**92 项通过**，没有失败或跳过。
- 协议校验：14 个夹具通过。
- GUI-free 端到端：工作区复用、手动子代理、TODO、5 种会话来源导入、SSH forward 配置保存通过。
- Rust TUI 恢复探针：只终止测试新建的临时 Host 后，TUI 自动拉起新 Host 并重新获取快照通过。
- 首轮已有的 Rust 270 项、Node 7 项、媒体 4 项及打包安装验收仍适用；本次未修改其生产代码。
- 本次复核时，迁入凭证覆盖的 599 个原文件哈希保持一致。此后原项目已执行 TUI 专属代码剔除，因此该记录表示迁移时的来源快照，不表示原项目当前目录保持不变。

这证明独立运行链路和已列出的功能检查通过；没有据此声称每个真实模型厂商、付费 API、远程设备或交互终端环境都已实测。

复核命令（仅用于与迁移前的完整源码快照比较，正常构建不需要它；当前 `expert_chat` 已剔除终端源码，不能作为该快照）：

```powershell
python tool/audit_migration.py --source PATH_TO_ORIGINAL_SNAPSHOT
```

结构化证据：`output/validation/completeness-audit.json`、`output/validation/tui-redesign-e2e.json`、`output/validation/tui-host-recovery.json`、`output/audit-dart-tests.log`。
