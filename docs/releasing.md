# 版本与发布

源码更新、生成本地安装包、发布 GitHub Release、部署 GitHub Pages 是独立步骤。成功的本地构建不代表已经公开发布。

## 当前版本入口

| 组件 | 版本来源 | 当前状态 |
| --- | --- | --- |
| `blogctl` | 根目录 `Cargo.toml` 的 `workspace.package.version` | 0.1.0。推送 `blogctl-v<版本>` 的 tag 后由 `blogctl-release.yml` 编译发布。 |
| 研究写作插件 | `extensions/research-blog/package.json` 与 lockfile | 源码 0.3.0；本地可打包，公开的历史 `vscode-v0.1.0` 安装包不含当前完整功能。 |
| Research Notes 主题 | `extensions/research-notes-theme/package.json` | 源码 0.1.0，可本地构建和打包。 |
| 静态站点 | main 的 Pages 工作流 | 仅在远端成功构建与部署后更新，不随本地 VSIX 安装自动发布。 |

## 准备可发布内容

1. 核实 Git 远端、发布账号、当前分支与待发布 commit；检查改动包含的是本次范围，保留作者现有工作。
2. 用 `blogctl init <空目录> ...` 新建站点，构建与预览，确认不依赖作者个人文章、CV、缓存或本机路径。`pages.yml` 在 CI 里也会走一遍。
3. 更新相应版本号与 lockfile、CHANGELOG 和下载入口。`blogctl`、插件和主题分别版本化。Zola 和 Typst 的版本号和哈希固定在 `crates/blog-site/src/tools.rs`，工作流的缓存键、AGENTS.md 和文档里也写了版本号，升级时要一起改。
4. 运行 [贡献指南](../CONTRIBUTING.md) 的相关检查，确认 ZEN 生成文件同步。涉及界面和 Zotero 时补充实际平台验证。
5. 核查待提交文件与安装包：不包含 `.env`、API 密钥、Zotero 二进制和私有库、PDF、联系人、测试缓存、草稿发布产物。保留 MIT 与实际 bundle 的第三方通知。

## blogctl

1. 改根目录 `Cargo.toml` 的 `version`，运行 `cargo build` 更新 `Cargo.lock`，提交。
2. 推送 tag `blogctl-v<版本>`，比如 `blogctl-v0.2.0`。`blogctl-release.yml` 检查 tag 和 `Cargo.toml` 一致，为 Windows（x64、ARM）、Linux（x64、ARM，musl 静态链接）、macOS（Apple 芯片、Intel）编译，上传压缩包和 `SHA256SUMS`。
3. 下载一个压缩包，核对校验和，运行 `blogctl --version`，再用它 `init` 一个新站点并 `check`。

新站点的 `pages.yml` 写着生成它的 `blogctl` 版本，CI 按这个版本下载。所以发布新版本不会改变已有站点的构建；站主要升级时改 `BLOGCTL_VERSION`。新版本的模板和样式不会自动进入已有站点。

## VS Code 插件

```powershell
npm --prefix extensions/research-blog ci --ignore-scripts
npm --prefix extensions/research-blog run build
npm --prefix extensions/research-blog test
npm --prefix extensions/research-blog run package
```

安装生成的 `research-blog-VERSION.vsix`，重载窗口，在真实站点检查命令、补全、预览、文献工作台与引用。确认 VSIX manifest 版本、内置文件、第三方许可证和测试使用的是同一份源码。

`vscode-extension.yml` 在 Windows 和 Linux 上构建测试并上传 Actions artifacts。Artifact 是该次构建结果，只有绑定明确版本与 commit 并正式上传到 Release 后，才作为稳定下载入口。发布插件可以沿用 `vscode-vVERSION` 的 tag 约定；发布说明应写最终能力、必要升级步骤、测试范围与已知限制。项目尚未发布到 VS Code Marketplace。

主题使用独立目录与版本：

```powershell
npm --prefix extensions/research-notes-theme ci --ignore-scripts
npm --prefix extensions/research-notes-theme run build
npm --prefix extensions/research-notes-theme run package
```

重新打包后才能更新已安装主题。ZEN 修改同时影响插件中的图形片段和 AI 规则，需判断两个包是否都需要发布。

## GitHub Pages

先运行 `blogctl check`。核对 `site/config.toml` 的 `base_url`，并在仓库 Settings → Pages 中选择 GitHub Actions。推送 main 会触发 `pages.yml`；PR 只验证，不部署。

等待构建和部署成功后，打开实际站点检查资源子路径、文章链接、公式、图形、文献回链、搜索、RSS、手机排版和明暗主题。确认草稿、预览编辑按钮、会话配置、文献库与私密材料未公开。若发布失败，保持“待发布”状态，修复并重新验证，不把本地成功记录为上线。

## 发布记录

Release 附上版本、对应 commit、安装包和简短完整演示：修改文章并看实时预览 → 修改 ZEN 并看风格同步 → Zotero 批注生成草稿 → 插入引用 → 正式静态构建。说明使用的是演示库还是用户真实库，并记录平台和仍需用户完成的 Zotero 授权/登录步骤。

公开推广前，应以新用户实际生成站点、安装插件并完成第一篇文章的结果为依据，其中至少一次只靠 Claude Code 或 Codex 按 README 和 AGENTS.md 的指令完成，不手动补步骤。不得将本机已安装版本、历史下载包或 CI 打包成功混为同一项完成记录。
