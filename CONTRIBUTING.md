# 参与贡献

欢迎提交修复、使用反馈和文档改进。这个项目的重点是本地写作和静态发布；实时预览、ZEN 风格、文献功能，以及引用能追溯到原文，都是核心功能。

## 开始开发

阅读 [入门](docs/getting-started.md) 与 [架构](docs/architecture.md)。`AGENTS.md` 写了文件地图、修改规则和每种改动的验证命令；用 agent 改代码时让它先读，人也可以当速查表。先用 `cargo run -p blogctl -- init .tools/demo --name "演示" --url "https://example.invalid/notes/"` 创建自己的演示站点，避免修改仓库中的作者文章和简历。

开发需要 Rust stable 工具链和能显示中文的字体。Zola 0.23.4 和 Typst 0.15.1 由 `blogctl` 自己下载。改插件和前端测试另外需要 Node.js 18+：

```powershell
npm --prefix extensions/research-blog ci --ignore-scripts
npm --prefix extensions/research-blog run build
```

CI 使用 Node.js 22，并安装 Noto CJK 字体。

报告问题时附上系统、工具版本、复现步骤、预期与实际结果、最小示例和已脱敏日志。功能提议应说明研究写作中的具体需求与使用流程。大型变更先开 Issue 讨论范围，简单修复可以直接提交 PR。Issue 和 PR 均接受中文或英文。

## 修改原则

- 修改风格先编辑 `ZEN.md`，运行同步；不要直接修改生成的色值、主题 JSON、模板参数和规则。新增组件尽量复用语义变量。
- Markdown 转换在临时副本中进行，避免改写用户文章。公式、代码、转义字符和中文路径需要保持正确。
- 草稿不能进入正式输出；私有文献、附件路径、API 密钥与预览凭据不能写入公开目录。
- 文献修改遵守 Zotero 版本与授权约束，不覆盖其他客户端已经修改的条目。
- 遵循已有交互：保留滚动与键盘操作，支持明暗主题、手机宽度与减少动态效果。
- AI 生成内容由作者检查。不要在示例或测试中伪造研究结果、论文状态或来源。

新增功能应提供对应最小演示或必要的回归测试。文档和低影响的样式调整通常用链接检查、构建与视觉验证即可；测试应覆盖行为和真实错误，不只重复实现。

## 验证

以下测试与 CI 说明针对完整的工具源码仓库；生成的个人站点日常使用 `blogctl check`。按实际改动运行相关检查，提交涉及多模块的工具变更前使用：

```powershell
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo run -p blogctl -- zen --check
cargo run -p blogctl -- check
node --test tests/*.test.cjs
npm --prefix extensions/research-blog test
npm --prefix extensions/research-blog run package
```

`cargo test` 里有一组对照测试（`crates/blog-site/tests/parity.rs`），用移植前 Python 版的输出检查公式保护、引用排版和 BibTeX 解析，改动这些行为时要说明原因。

涉及预览时，在隔离站点运行 `blogctl preview`，检查 Markdown、公式、Typst 与 ZEN 更新、滚动位置、错误恢复及草稿边界。新建站点的流程用 `blogctl init <空目录> ...` 再 `blogctl --root <目录> check` 检查。涉及 Zotero 时用独立测试库，检查读写授权、PDF、批注、版本冲突和离线行为；不要把自己的文献库或密钥提交为 fixture。涉及界面时核查深浅主题、窄屏、键盘与文本溢出，并说明验证平台。

现有工作流分工：

| 工作流 | 实际执行 |
| --- | --- |
| `pages.yml` | 从源码编译 `blogctl`，Node 前端测试，`blogctl check`，再新建一个演示站点构建检查；main 的成功构建发布 Pages，PR 仅检查。 |
| `rust.yml` | Windows 和 Linux 上的 Rust 格式、测试、Clippy 和 ZEN 检查；按 Rust 代码和打包进 `blogctl` 的文件路径触发。 |
| `blogctl-release.yml` | 推送 `blogctl-v<版本>` 时为六个平台编译并发布 `blogctl`。 |
| `vscode-extension.yml` | Windows/Linux 的插件依赖安装、构建、测试、ZEN 检查与 VSIX 打包，上传 Actions artifact。 |

CI 通过不等于在所有系统和文献库上都完成实际使用验证。PR 里写明测试结果与仍未验证的范围。

## 提交 PR

标题写具体问题或新行为。说明触发条件、修改后效果、验证方式；界面改动可以附截图。不要提交 `.env`、`.tools`、缓存、构建目录、私有 PDF、联系人信息或整套个人内容。版本号、安装包与 Release 由维护者按 [发布流程](docs/releasing.md) 统一更新。

贡献的项目代码、文档和可复用演示按 [MIT](LICENSE) 提供。保留已有第三方通知；提交第三方素材必须注明来源与允许使用的许可。个人文章、CV 和研究图片的授权不因工具开源而改变，详见 [CONTENT_LICENSE](CONTENT_LICENSE.md)。
