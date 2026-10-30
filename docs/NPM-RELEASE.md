# npm 发布流程

本仓库承接已有的 `@wweiyi/expert` 包，命令继续为 `expert`。npm 元数据中的源码地址改为 `https://github.com/wweiyi2004/expert_tui`，更新器仍按同一包名查询 npm registry。

**当前只准备流程。** 仓库版本保持 `0.2.0`；该版本已经在 npm 发布，不能覆盖。本次不创建标签、GitHub Release 或上传 npm。旧 npm 页面上的 repository 元数据会在未来新版本发布后更新。

## 触发方式

工作流：`.github/workflows/npm-release.yml`。

- 普通 `git push` 不触发发布。
- 手动运行工作流时，`publish` 默认 `false`，只构建、测试和上传 Actions 构建产物。
- 发布 GitHub Release，或从对应版本标签手动运行且明确选择 `publish=true`，才会进入 npm 发布步骤。
- 发布前检查标签必须等于 `v` 加源码版本，Rust/npm 版本必须一致，且该版本尚未存在于 npm。分支构建和重复版本都会被拒绝。

Windows job 使用 Node.js 22、Dart 3.12、Rust 和 Python 3.12 从本仓库构建完整运行包，再运行 Dart、Rust、Node、媒体、Host、独立启动和真实本地安装/升级验收。npm tarball、ZIP 和校验记录作为 Actions artifact 保存。

Ubuntu job 仅在明确发布时运行，下载已验证的同一 TGZ，重新校验包身份、源仓库、SHA-256、SHA-512、文件清单及每个文件的摘要，然后通过 npm OIDC 上传，并核对 registry 返回的 integrity。

## 首次正式发布前的一次性 npm 配置

当前 npm CLI 登录已失效，npm 端 Trusted Publisher 尚未完成配置；无需现在登录或配置。准备实际发布时，在已有包 `@wweiyi/expert` 的设置中添加 GitHub Actions Trusted Publisher：

| 字段 | 值 |
| --- | --- |
| Organization or user | `wweiyi2004` |
| Repository | `expert_tui` |
| Workflow filename | `npm-release.yml` |
| Environment | 留空 |
| Allowed action | 允许 `npm publish` |

此流程不需要向 GitHub 保存长期 `NPM_TOKEN`。npm 要求工作流的 `id-token: write` 权限及正确匹配的 repository 元数据；新建的 Trusted Publisher 需要在两天内首次成功发布，因此应在准备正式发版时配置。[npm 官方说明](https://docs.npmjs.com/trusted-publishers/)

也可在届时重新登录后，用 npm 11.15+ 配置：

```powershell
npm login --registry=https://registry.npmjs.org --auth-type=web
npm trust github @wweiyi/expert --repo wweiyi2004/expert_tui --file npm-release.yml --allow-publish --yes
```

## 有实际更新后的发布步骤

1. 同步递增 `next/tui/Cargo.toml` 和 `npm/expert/package.json`，更新 Cargo lockfile，完成验证后提交。
2. 创建与版本一致的 `vX.Y.Z` 标签并推送，在 GitHub 发布该标签的 Release。
3. 工作流验证成功后才上传 npm。以 registry 上精确版本和 integrity 的复核结果判断发布成功。

发布前可独立验证身份和待发布性（只读取 registry，不上传）：

```powershell
python tool/release.py metadata
python tool/release.py metadata --check-unpublished
```

当前第二条命令应拒绝已发布的 `0.2.0`，这是预期保护。

## 旧用户数据

包名、命令和更新协议保持兼容。独立项目的默认数据目录已与 GUI 分离；从拆分前版本升级时，不会自动把 GUI 会话、凭据和配置复制到新目录。需要旧资料时使用显式备份/设置导入入口。此目录边界与源码迁移时的决定一致。
