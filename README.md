# Expert TUI

独立的终端 AI 助手项目，迁自 `expert_chat` 当前工作区。包含 Rust TUI、纯 Dart 任务宿主、Rust 文件工具和 Python 文档媒体 worker。

源码仓库：[wweiyi2004/expert_tui](https://github.com/wweiyi2004/expert_tui)。

当前完整发行包支持 **Windows x64**。开发需要 Rust（最低 1.88）、Dart 3.12+、Python 3.12、uv、Node.js 20+（本次 npm 验收使用 22.23.4）和 Windows C++ 构建工具。无需安装 Flutter SDK；本机使用 Flutter 自带的 Dart 也可以。

## 构建与运行

在项目根目录执行：

```powershell
git clone https://github.com/wweiyi2004/expert_tui.git
cd expert_tui
dart pub get --enforce-lockfile
uv sync --project server --frozen
python tool/build_native.py
.\output\native\bundle\expert-tui.exe
```

程序保留启动时的工作目录。可以从你的代码目录调用上述 EXE 的绝对路径。

生成带 Node.js 启动器和更新器的本地预览包：

```powershell
python tool/build_tui_npm_release.py --draft --skip-build
```

产物位于 `output/releases/`，包含便携 ZIP、npm TGZ 和校验清单。解压 ZIP 后运行 `expert.cmd`。从 TGZ 安装时命令为 `expert-tui`，避免覆盖原项目的 `expert` 命令。

新包身份为 `@wweiyi/expert-tui`，尚未由本次迁移发布。`--draft` 生成 private 包；正式发布需另行安排。更新器只查询自身包名，不会安装 `@wweiyi/expert` 的旧发行包。

## 数据与配置

- Host 数据：`%LOCALAPPDATA%/expert-tui/state/`
- 数据库：上述目录内的 `documents/expert_chat.sqlite`（保留兼容文件名）
- 桥接发现：上述目录内的 `host/discovery/`
- 界面配置与宠物：`%APPDATA%/expert-tui/`
- npm 版本缓存：`%LOCALAPPDATA%/expert-tui/cli/`
- 独立运行文件工具时的默认存储：`%LOCALAPPDATA%/expert-tui/desktop/`

支持 `EXPERT_HOST_DATA_DIR`、`EXPERT_HOST_DISCOVERY`、`EXPERT_TUI_CONFIG`、`EXPERT_CLI_HOME` 等原有显式覆盖项。新项目默认不读取或迁移 GUI 的会话、密钥与配置。旧数据可通过兼容备份/设置导入入口显式导入；不要让独立升级的两个项目同时写同一资料目录。

## 验证

```powershell
dart analyze
python tool/test_runtime.py
cargo test --manifest-path next/Cargo.toml --release --locked
node --test npm/expert/test/*.test.cjs
.\server\.venv\Scripts\python.exe -m unittest discover -s server -p test_desktop_media_worker.py
.\server\.venv\Scripts\python.exe tool/host_e2e.py --bundle output/native/bundle --report output/validation/host-e2e.json
```

Host 端到端验收使用临时目录和回环假模型，不调用付费服务。生成 npm 包后，还可运行 `node tool/test_tui_npm_release.cjs` 验证本地安装和升级。

`test_runtime.py` 将图片集成测试指向本仓库构建的媒体 worker；需先完成原生 bundle 构建。

本次验证的结果及本机 Node.js 版本差异见 [验证记录](docs/VALIDATION.md)。
逐文件迁移范围、补齐记录和 GUI 排除项见 [完整性复核](docs/COMPLETENESS.md)。

## 目录

| 路径 | 用途 |
| --- | --- |
| `next/tui` | 终端界面及 headless 模式 |
| `next/desktop` | 工作区文件工具 |
| `bin`、`lib` | 纯 Dart Host、任务执行和存储 |
| `third_party/xterm` | 保留 MIT 许可证的纯 Dart 终端核心子集 |
| `server` | 最小媒体 worker 及独立 Python 依赖锁 |
| `npm/expert` | Node.js 启动器、缓存和更新器 |
| `design/tui` | 编译所需夹具、宠物资源和迁移时设计资料 |
| `tool` | 当前项目独立构建及验收脚本 |

`expert_chat` 中的 TUI 专属客户端、资源和构建入口已剔除；GUI 仍需使用的 Host 和任务内核保留。两个项目各自维护同源的内核实现，目前没有共享包或跨仓库源码依赖。原有数据格式、MCP 标识及部分兼容命名予以保留。详细来源见 [迁移记录](docs/EXTRACTION.md)；许可沿用根目录 `LICENSE`。
