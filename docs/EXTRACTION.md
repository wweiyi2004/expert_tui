# 源码迁移记录

目标目录：`expert_tui`。来源：`expert_chat` 迁移时的当前工作区，包含其尚未提交的修改，未仅从 Git HEAD 导出。

`extraction-manifest.json` 保存来源提交、迁入文件在原项目中的 SHA-256 以及保留的 Dart 测试列表。它是来源凭证，不是修改后新项目的发行校验清单。

本次迁移：

1. 按 Host 入口的实际 import/export/part 依赖提取 Dart 源码；将条件导入固定为独立 Host 使用的原生分支，包名改为 `expert_tui`。
2. 从已有 xterm 副本中仅提取纯 Dart 终端核心，保留原许可证，去除 Flutter 清单依赖。
3. 迁入 Rust workspace、终端夹具、宠物资源、文件工具、数学脚本和最小媒体 worker。
4. 将构建改为仅使用本仓库源码；媒体 worker 在本项目重新编译，打包不借用旧 GUI 发行目录。
5. 将 Host、桥接发现、界面配置、文件工具和版本缓存的默认目录改为 `expert-tui` 命名空间。保留原有显式环境变量覆盖。
6. npm 包身份改为 `@wweiyi/expert-tui`，安装命令改为 `expert-tui`，不自动发布或全局安装。

现阶段保留 Rust + Dart + Python 的既有架构。没有迁入 GUI、移动平台工程、账户配置、原项目缓存或原项目发布二进制。

部分 Host 协议仍保留 GUI 客户端接口及学习/创作数据模型，便于沿用执行与存储逻辑；它们不引入 Flutter。GUI 专属操作在没有相应客户端时继续返回能力不可用。

迁移后的后续清理已从 `expert_chat` 移除 TUI 专属客户端、资源、启动入口和打包脚本，并保留 GUI 使用的 Host/任务内核。按最终范围，不抽取共享包；两个项目各自维护同源实现。
