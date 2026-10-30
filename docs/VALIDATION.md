# 迁移验证

日期：2026-10-30。平台：Windows x64。

| 检查 | 结果 |
| --- | --- |
| `dart analyze` | 无问题 |
| `python tool/test_runtime.py` | 补齐后 92 项通过，包含独立存储和真实图片/音视频检查 |
| Rust workspace release 测试 | 270 项通过 |
| Node 更新器测试 | 7 项通过 |
| 媒体 worker 测试 | 4 项通过 |
| 完整源码构建 | Rust TUI、Rust 文件工具、Dart CLI bundle、Python 媒体 worker 全部成功 |
| 独立首次启动 | 临时 LOCALAPPDATA 中新建独立数据库，默认路径正确，空闲退出成功 |
| Host 端到端 | 数据目录独占、客户端权限、短暂断连恢复、持久化恢复、拒绝重复副作用、进程崩溃恢复通过 |
| npm 安装/升级端到端 | Node.js 22.23.4 下真实本地安装、离线渲染、回环 registry 升级、旧 Host 运行时升级及原生更新桥接全部通过 |
| 付费模型请求 | 0；端到端使用本地假模型 |
| 源项目完整性 | 补齐后的迁入清单对应的 599 个来源文件哈希未改变 |
| 源码依赖检查 | 无 Flutter 包，无指向 `expert_chat` 的包根或应用代码导入 |

详细报告位于 `output/validation/`，源码构建日志与测试日志位于 `output/`。发行文件及摘要由 `output/tui-npm-build/latest.json` 指向。

逐文件范围、首轮遗漏及补齐结果见 [完整性复核](COMPLETENESS.md)。

## Node.js 环境差异

本机系统 Node.js 24.14.1 解压生成的 TGZ 时报告 `Z_DATA_ERROR`；同一文件通过 Python 的完整 gzip 校验和 ZIP CRC 校验。Node.js 22.23.4 能正确解压同一 TGZ。

为完成安装/更新验收，单独下载官方 Node.js 22.23.4 Windows x64 压缩包，核对官方 SHA-256 后解压到被 Git 忽略的 `.tools/node22/`。下载 URL、SHA-256 和本地程序位置记录在 `output/node22.json`。系统 Node、系统 PATH 和全局 npm 安装未修改。

Node 官方仓库中有 [Windows 上相似的 24/26 解压问题报告](https://github.com/nodejs/node/issues/66465)。这只是相关线索，未据此断言本机故障原因。此机器进行 npm 安装/升级时使用经本次验证的 Node.js 22。原生 bundle 的普通运行无需 Node.js，npm 启动器和更新器需要它。
