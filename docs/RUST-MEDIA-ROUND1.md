# 第一轮 Rust 媒体替换

本轮只替换图片预处理和 Zstd 会话解压。Dart 任务内核、PDF 解析、可选数学功能保持原架构。版本仍为 `0.2.0`；没有创建发布标签、GitHub Release 或上传 npm。

## 运行边界

| 操作 | 实现 | 成品位置 |
| --- | --- | --- |
| `image_resize` | Rust `image` | `expert-media-worker/expert-media-worker.exe` |
| `session_zstd` | Rust `zstd`（静态链接底层 zstd 原生库） | 同上 |
| `pdf_text` | Python `pypdf` | `expert-pdf-worker/expert-pdf-worker.exe` |

Dart `runMediaWorker` 按操作路由，仍通过临时 JSON 请求文件和 JSON 标准输出通信。Rust worker 不接受 PDF 操作，Python worker 不再包含图片或 Zstd 分支。原 `EXPERT_MEDIA_WORKER` 全操作覆盖项保留兼容，另增加 `EXPERT_RUST_MEDIA_WORKER` 和 `EXPERT_PDF_WORKER` 精确覆盖项。

PDF 的生产依赖不再包含 Pillow、zstandard、websockets；图片测试和会话 fixture 仍在开发依赖中使用这些包。PDF 打包时显式排除 PIL 和 zstandard，避免可选导入重新将它们带入成品。

## 行为和边界

- 小图不放大，不需要变换时返回 `changed=false`，Dart 保留原始文件字节。
- 需要处理时输出 PNG；保持比例、透明度、ICC 配置，应用 EXIF 方向后不再携带待重复执行的方向变换。支持 TUI 原有 PNG/JPEG/GIF/WebP/BMP 输入；动画处理首帧。
- 仍限制请求 24 MiB、二进制输入 15 MiB、图片 4000 万像素、边长参数 1–4096、PNG 输出 15 MiB。
- Zstd 仍读取首个 frame，并限制解压文本 16 MiB；另限制解码窗口为 256 MiB。非 UTF-8、校验损坏和截断流会失败，不返回部分会话。
- 以前 Python stream_reader 可能接受截断流并返回部分文本；本轮明确改为拒绝截断。这是边界收紧。
- worker 保留约 15 秒看门狗和 512 MiB 进程内存限制。Windows 使用 Job Object；资源限制不等于操作系统沙箱。
- Rust 和 Pillow 的缩放采样算法不要求逐像素相同；回归按方向、尺寸、透明度、色彩配置与协议行为验证。

## 验证

- Dart 全套 96 项通过，新增直接经过真实 worker 的图片、Zstd、PDF 路由和错误脱敏检查。
- Rust workspace 原有 270 项及新增 worker 4 项通过；新 crate 的 Clippy `-D warnings` 通过。
- 媒体跨语言验收 23 项通过：包含 8 种 EXIF 方向、PNG/WebP EXIF、透明度、ICC、动画首帧、解压边界、截断/校验/编码错误及实际打包 PDF 文本提取。
- 实际 AOT Host 经 RPC 导入图片并返回 TUI 预览通过：3200×1600 缩为 1536×768，透明度保留；刻意让 PDF worker 不可用时仍能处理图片。
- 5 种外部会话来源、子代理及 TODO 的原生 Host 端到端通过，包含真实 Rust Zstd 路径。
- Node 更新器 7 项、发布防护 10 项和工作流语法检查通过。本地安装/升级及包内文件验证另有结构化报告。

报告位于 `output/validation/rust-media-round1-*.json`，测试日志位于 `output/rust-media-round1-*.log`。测试只使用合成数据和回环假模型，付费请求为 0。

## 本机体积

| 内容 | 替换前 | 替换后 |
| --- | ---: | ---: |
| 图片/Zstd/PDF 组件合计 | 35.77 MiB | Rust 1.49 + Python PDF 21.71 = 23.20 MiB |
| 解压后的完整原生运行包 | 65.24 MiB | 52.68 MiB |
| npm TGZ | 约 28.59 MiB | 约 22.16 MiB |

完整原生包减少约 19%，npm 下载包减少约 22%。Python PDF 仍保留，所以本轮没有彻底移除 Python。

本机合成输入、每次独立启动 worker、排除一次预热后 5 次测量的中位数：3200×1600 纯色 PNG 缩为 1000×500 的单次调用从约 131 ms 降到 33 ms；560000 字节重复文本的 Zstd 解压从约 76 ms 降到 12 ms。它们包含进程启动和序列化，不代表整体 TUI、模型响应或所有图片的性能倍率。

## 构建和验证自定义目录

```powershell
python tool/build_native.py --output output/native-round1-check
python tool/test_runtime.py --bundle output/native-round1-check/bundle
$env:EXPERT_TEST_MEDIA_BINARY = (Resolve-Path output/native-round1-check/bundle/expert-media-worker/expert-media-worker.exe).Path
$env:EXPERT_TEST_PDF_BINARY = (Resolve-Path output/native-round1-check/bundle/expert-pdf-worker/expert-pdf-worker.exe).Path
.\server\.venv\Scripts\python.exe -m unittest discover -s server -p test_desktop_media_worker.py
python tool/build_tui_npm_release.py --draft --skip-build --native-dir output/native-round1-check
```

选择尚不存在 `bundle` 的新输出目录，旧成品及运行中的版本保持原样。npm 校验清单现在要求两个 worker 齐全，拒绝把旧的 Python 综合 worker 当作 Rust 新包使用。
