# 给 agent 的说明

This file is for coding agents such as Claude Code and Codex. It is written in Chinese, like the rest of the project. The commands are the same on every platform unless a line says otherwise. People can use it as a quick reference too.

## 这是什么

Research Blog 是一套个人博客和写作方案：Zola 静态网站、Python 构建和预览脚本、一个 VS Code 写作插件、一个编辑器配色主题，以及一个叫 `ZEN.md` 的风格文件。适合写公式、画图、引文献的笔记。安装、改样式、加功能都可以由 agent 来做，前提是按这里写的规则改，改完跑对应的检查。

这个目录是工具的源码仓库，同时也是作者自己的网站。给别人用的新站点由 `python scripts/init_site.py` 生成，新站点里有自己的 AGENTS.md。

## 先分清两种目录

- **源码仓库，也就是这里。** `site/content/posts/` 里是作者的文章，`profile/` 是作者的简历源，`site/config.toml` 里是作者的站名和地址。这些是作者的内容，没有明确要求时不要改、不要删、不要当成示例内容。
- **要做试验或演示时，先生成一个新站点。** 比如 `python scripts/init_site.py .tools/demo --name "演示" --author "Researcher" --url "https://example.invalid/notes/"`。目标目录必须不存在或为空；`.tools/` 被 Git 忽略。

## 文件地图

| 位置 | 作用 |
| --- | --- |
| `ZEN.md` | 风格的唯一来源：`zen:tokens` 和 `zen:prompts` 两段 JSON。颜色、字体、版面、间距、图形线宽、AI 规则都在这里改。 |
| `scripts/sync_zen.cjs` | 从 `ZEN.md` 生成 `site/static/zen.css`、`design/zen.generated.cjs`、两套编辑器主题和主题预览页、Typst 模板里的参数段、插件的 `blogdiagram` 片段。`--check` 只检查不写。 |
| `scripts/build_site.py` | 同步 ZEN，编译文章图形，构建公开网站到 `site/public/`。`--base-url` 可以临时换地址。 |
| `scripts/check_site.py` | 检查 `site/public/` 的链接、目录锚点、搜索索引、RSS、站点地图，以及不该公开的内容。 |
| `scripts/editor.py` | `new "标题"` 新建草稿；`preview` 实时预览，含草稿，地址打印在终端，加 `--no-open` 不开浏览器；`check` 构建并检查。 |
| `scripts/setup_tools.py` | 检查 Python 和 Node.js，把 Zola 0.23.4 和 Typst 0.15.1 下载到 `.tools/` 并校验哈希。`--check` 只报告。 |
| `scripts/init_site.py` | 从 `examples/starter` 和共享源码生成一个干净的新站点。 |
| `scripts/check_starter.py --destination <空目录>` | 完整走一遍新站点流程：初始化、安装依赖、构建、预览、ZEN 同步。 |
| `scripts/zotero.py` | 启动项目私有的 Zotero 客户端，只在 Windows 上用。 |
| `scripts/prepare_site.py`、`math_markdown.py`、`references.py`、`zen.py`、`site_features.py` | 构建副本、公式保护、引用排版、ZEN 读取、工具查找。由上面的入口调用，一般不直接运行。 |
| `site/config.toml` | 站名、作者、`base_url`、主题树 `topic_tree`、标签中文名 `tag_labels`、`profile_enabled`、`agent_endpoint`。 |
| `site/content/posts/<目录>/index.md` | 文章。图形源码在同目录的 `figures/*.typ`，图片在 `assets/`。 |
| `site/data/collections.json` | 系列和分组。`site/data/references.json` 是公开书目。 |
| `site/templates/` | Tera 模板。`site/static/` 是样式和前端脚本，其中 `zen.css` 是生成的。 |
| `extensions/research-blog/` | VS Code 写作插件：`src/*.cjs` 源码，`test/` 测试，`npm run build` 同步 ZEN 并打包到 `dist/`。 |
| `extensions/research-notes-theme/` | 编辑器配色主题，`themes/*.json` 由 ZEN 生成。 |
| `design/visual-system/typst/blog-diagram.typ` | 图形模板，参数段由 ZEN 生成。 |
| `crates/` | 可选的 Rust：文章解析、Tantivy 索引、本地文章助手 API、`blogctl`。网站和插件不依赖它。 |
| `docs/`、`tests/` | 文档；Python `unittest` 和 `node --test` 测试。 |
| `.tools/`、`output/`、`target/`、`site/public/`、`node_modules/` | 生成物和缓存，Git 忽略。不要手改，不要提交。 |

## 改东西的规则

**风格。** 只改 `ZEN.md` 里的两段 JSON，然后运行 `node scripts/sync_zen.cjs`。不要手改任何生成文件。保留 `schemaVersion`、`graphite` 和 `paper` 两套调色板的键名，以及两段 JSON 的标记注释。改背景色时一并看 `effects`、`contrast` 和 `imageWords`。组件的布局细节改 `site/static/` 里的 CSS，颜色用 `zen.css` 的变量，比如 `--accent`、`--text-primary`、`--surface-0`，不要写死色值。改完检查深浅两套主题、窄屏、键盘操作和 `prefers-reduced-motion`。

**文章。** 文章是 `site/content/posts/<目录>/index.md`，目录名就是网址。开头是 TOML：`title`、`date`、`description`、`draft`，`[taxonomies]` 下的 `tags`，`[extra]` 下可选的 `author` 和 `aliases`。正文是普通 Markdown：

- 公式直接写 `$x$` 和成对的 `$$` 独立行，不要加 HTML 包装，不要给 `_`、`*` 加转义；字面美元写 `\$`。代码块里的 `$` 不会渲染。
- 双链写 `[[文章标题]]`、`[[文章标题|显示文字]]`、`[[文章标题#章节标题]]`。
- 引用写 `[@citekey]`，citekey 必须在 `site/data/references.json` 里。构建会生成文末的参考文献和全站文献页。
- 图形：源码放在 `figures/<id>.typ`，正文写 `{{ <fig id="<id>" caption="图标题" alt="描述" /> }}`，构建时编译成同目录的 `<id>.svg`。模板在 `design/visual-system/typst/blog-diagram.typ`，复制过去用，不要自己发明依赖。
- `draft = true` 的文章不进公开构建，预览里能看到。提交到公开 Git 仓库的草稿别人照样能看到。

**书目。** 手工往 `references.json` 加条目时只能用这些字段：`id`、`citekey`、`title`、`author`、`issued`、`type`、`container-title`、`DOI`、`URL`、`volume`、`issue`、`page`、`publisher`。`author` 是 `[{"family": "Mikolov", "given": "Tomas"}]` 或 `[{"literal": "Zotero"}]`，`issued` 是 `{"date-parts": [[2013]]}`。别的字段会让构建失败。连上 Zotero 的用户由插件的「更新博客引用数据」维护这个文件。

**系列和主题。** 系列、分组和文章顺序在 `site/data/collections.json`；主题页的层级在 `site/config.toml` 的 `topic_tree`，标签的中文名在 `tag_labels`。

**构建。** 一律用 `python scripts/build_site.py` 或 `python scripts/editor.py check`。直接运行 `zola build` 或 `zola serve` 会跳过公式保护和引用排版，结果不对。

**插件。** 改了 `extensions/research-blog/src/` 之后运行 `npm --prefix extensions/research-blog test` 和 `npm --prefix extensions/research-blog run build`。要装进 VS Code，先 `npm --prefix extensions/research-blog run package`，再从 VSIX 安装。装好的插件不读仓库里的文件，所以 ZEN 改了之后要重新打包安装才会变。主题同理，目录是 `extensions/research-notes-theme`。

**文献工作台。** 它是 VS Code 里的界面，通过 `http://127.0.0.1:23119/api/` 连本机 Zotero。agent 驱动不了它，也不要绕过它去读 Zotero 的数据库。

**隐私。** `site/` 下不放密钥、PDF、Zotero 数据和联系方式。`.env`、`.tools/`、`output/`、`target/`、`site/public/`、`*.vsix` 都在 `.gitignore` 里，不要提交。`check_site.py` 检查已知的泄露规则，但不能替人审查文章内容。

**Rust。** 只有改 `crates/` 时才需要 Rust。改完运行 `cargo fmt --all -- --check`、`cargo test --workspace --locked`、`cargo clippy --workspace --all-targets --locked -- -D warnings`。

## 每种改动怎么验证

| 改了什么 | 运行 |
| --- | --- |
| `ZEN.md` | `node scripts/sync_zen.cjs`，然后 `node scripts/sync_zen.cjs --check` |
| 文章、书目、系列、`config.toml` | `python scripts/editor.py check` |
| 模板、CSS、前端脚本 | `python scripts/editor.py check` 和 `node --test tests/*.test.cjs` |
| `scripts/*.py` | `python -m unittest discover -s tests -p "test_*.py"`，再 `python scripts/editor.py check` |
| 插件源码 | `npm --prefix extensions/research-blog test` 和 `npm --prefix extensions/research-blog run build` |
| `examples/starter/` 或 `init_site.py` | `python scripts/check_starter.py --destination .tools/starter-review`，目标目录须不存在或为空 |
| `crates/` | 上面的三条 cargo 命令 |

预览的行为用 `python scripts/editor.py preview --no-open` 验证：终端会打印地址，改文件后用 HTTP 请求确认页面更新，结束时停掉进程。

## 环境

需要 Python 3.11+、Node.js 18+、Zola 0.23.4、Typst 0.15.1 和一种中文字体。Python 脚本只用标准库。

```sh
python scripts/setup_tools.py                     # 检查，缺什么就下载到 .tools/
npm --prefix extensions/research-blog ci --ignore-scripts
npm --prefix extensions/research-blog run build   # 预览用的本地编辑服务
```

构建和预览脚本按这个顺序找 Zola 和 Typst：环境变量 `ZOLA`、`TYPST`，仓库的 `.tools/zola/` 和 `.tools/typst/`，然后 PATH。所以 `.tools/` 里的版本会盖过系统里不对的版本。完整流程在 Windows 上验证过，CI 在 Linux 上构建，macOS 还没有人走过。

## 已知的坑

- Zola 和 Typst 的版本要精确到上面两个号，模板和 shortcode 的行为随版本变。
- Typst 第一次编译图形会联网下载 CeTZ 包，没有网络时图形编译会失败。
- Linux 需要中文字体，CI 装的是 `fonts-noto-cjk`。没有中文字体时 Typst 图里的中文是方块。
- `base_url` 可以带子路径，所有站内链接由它生成。
- 预览占用 1111 到 1120 之间第一个空闲端口。第一次预览之前必须先完成上面的 `npm ci` 和 `npm run build`。
- `site/content/_index.md` 的 `sort_by = "none"` 不能改成按日期，否则没有日期的页面会消失。
- 文章目录可以用中文名。路径处理一律用 `pathlib`，不要手拼字符串。
- 公式里的 `\`、`aligned`、矩阵要原样保留，不要"修正"反斜杠。
- `[[链接]]`、`[@引用]` 和公式在代码块、行内代码里都不生效，这是有意的。
- 插件的 `npm ci` 要加 `--ignore-scripts`，CI 也是这么装的。

## 要先问人的事

- 推送到 `main`。这会触发 GitHub Pages 部署。
- 删除或重写作者的文章、简历、`site/data/` 里的数据，或者任何已经发布的内容。
- 改 `base_url`、插件 ID、包名、版本号、LICENSE。
- 需要密钥、账号，或者会向外部服务发送内容的事，包括 AI 补全和生图。
- 改 `ZEN.md` 的 schema、键名或两段 JSON 的标记。
- 升级 Zola、Typst、Node 这类大的依赖变更。

## 写文案的规矩

网站、README、提交说明和界面文字都用平实的中文：说清楚做什么、怎么用，短句。不用口号式的对仗句，不用「让……」「从……到……再到……」这类句式，不堆形容词。命令、路径、版本号要写准。中英对照时中文在前，英文在下。示例和测试里不要伪造研究结果、论文或引用，演示数据要标明是演示。设计上遵守 ZEN：黑白灰为主，细线，留白，红色只点一下。
