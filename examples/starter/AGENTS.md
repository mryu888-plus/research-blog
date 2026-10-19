# 给 agent 的说明

This file is for coding agents such as Claude Code and Codex. It is written in Chinese. The commands are the same on every platform unless a line says otherwise. The site owner can use it as a quick reference too.

## 这是什么

这个目录是一个用 Research Blog 模板生成的个人博客：Zola 静态网站、Python 构建和预览脚本、VS Code 写作插件和编辑器主题的源码，以及风格文件 `ZEN.md`。文章用 Markdown 写，公式直接写在正文里，图用 Typst 源码维护，引用来自 `site/data/references.json`，发布到 GitHub Pages。

站主可能让你做的事：装工具、改颜色和字体、新建文章和系列、画一张图、加一条引用、开预览、构建检查、准备发布。每种事下面都写了改哪里、怎么验证。

## 文件地图

| 位置 | 作用 |
| --- | --- |
| `ZEN.md` | 风格的唯一来源：`zen:tokens` 和 `zen:prompts` 两段 JSON。颜色、字体、版面、间距、图形线宽、AI 规则都在这里改。 |
| `scripts/sync_zen.cjs` | 从 `ZEN.md` 生成 `site/static/zen.css`、`design/zen.generated.cjs`、两套编辑器主题和主题预览页、Typst 模板里的参数段、插件的 `blogdiagram` 片段。`--check` 只检查不写。 |
| `scripts/build_site.py` | 同步 ZEN，编译文章图形，构建公开网站到 `site/public/`。`--base-url` 可以临时换地址。 |
| `scripts/check_site.py` | 检查 `site/public/` 的链接、目录锚点、搜索索引、RSS、站点地图，以及不该公开的内容。 |
| `scripts/editor.py` | `new "标题"` 新建草稿；`preview` 实时预览，含草稿，地址打印在终端，加 `--no-open` 不开浏览器；`check` 构建并检查。 |
| `scripts/setup_tools.py` | 检查 Python 和 Node.js，把 Zola 0.23.4 和 Typst 0.15.1 下载到 `.tools/` 并校验哈希。`--check` 只报告。 |
| `scripts/init_site.py` | 用这个目录当模板，再生成一个干净的站点。 |
| `scripts/zotero.py` | 启动项目私有的 Zotero 客户端，只在 Windows 上用。 |
| `site/config.toml` | 站名、作者、`base_url`、主题树 `topic_tree`、标签中文名 `tag_labels`。 |
| `site/content/posts/<目录>/index.md` | 文章。图形源码在同目录的 `figures/*.typ`，图片在 `assets/`。 |
| `site/content/about.md` | 关于页，普通 Markdown。 |
| `site/data/collections.json` | 系列和分组。`site/data/references.json` 是公开书目。 |
| `site/templates/` | Tera 模板。`site/static/` 是样式和前端脚本，其中 `zen.css` 是生成的。 |
| `extensions/research-blog/` | VS Code 写作插件源码。`npm run build` 同步 ZEN 并打包，`npm run package` 生成 VSIX。 |
| `extensions/research-notes-theme/` | 编辑器配色主题，`themes/*.json` 由 ZEN 生成。 |
| `design/visual-system/typst/blog-diagram.typ` | 图形模板，参数段由 ZEN 生成。 |
| `.github/workflows/pages.yml` | 推送 `main` 时构建、检查并发布到 GitHub Pages；PR 只检查。 |
| `.vscode/tasks.json` | VS Code 里的预览、检查和新建文章任务。 |
| `crates/` | 可选的 Rust 工具和文章助手。网站和插件不依赖它。 |
| `.tools/`、`output/`、`target/`、`site/public/`、`node_modules/` | 生成物和缓存，Git 忽略。不要手改，不要提交。 |

## 改东西的规则

**风格。** 只改 `ZEN.md` 里的两段 JSON，然后运行 `node scripts/sync_zen.cjs`。不要手改任何生成文件。保留 `schemaVersion`、`graphite` 和 `paper` 两套调色板的键名，以及两段 JSON 的标记注释。改背景色时一并看 `effects`、`contrast` 和 `imageWords`。组件的布局细节改 `site/static/` 里的 CSS，颜色用 `zen.css` 的变量，比如 `--accent`、`--text-primary`、`--surface-0`，不要写死色值。改完检查深浅两套主题、窄屏、键盘操作和 `prefers-reduced-motion`。

**文章。** 文章是 `site/content/posts/<目录>/index.md`，目录名就是网址。`python scripts/editor.py new "标题"` 会建好目录和草稿。开头是 TOML：`title`、`date`、`description`、`draft`，`[taxonomies]` 下的 `tags`，`[extra]` 下可选的 `author` 和 `aliases`。正文是普通 Markdown：

- 公式直接写 `$x$` 和成对的 `$$` 独立行，不要加 HTML 包装，不要给 `_`、`*` 加转义；字面美元写 `\$`。代码块里的 `$` 不会渲染。
- 双链写 `[[文章标题]]`、`[[文章标题|显示文字]]`、`[[文章标题#章节标题]]`。
- 引用写 `[@citekey]`，citekey 必须在 `site/data/references.json` 里。构建会生成文末的参考文献和全站文献页。
- 图形：源码放在 `figures/<id>.typ`，正文写 `{{ <fig id="<id>" caption="图标题" alt="描述" /> }}`，构建时编译成同目录的 `<id>.svg`。模板在 `design/visual-system/typst/blog-diagram.typ`，复制过去用，不要自己发明依赖。
- `draft = true` 的文章不进公开构建，预览里能看到。提交到公开 Git 仓库的草稿别人照样能看到。

**书目。** 手工往 `references.json` 加条目时只能用这些字段：`id`、`citekey`、`title`、`author`、`issued`、`type`、`container-title`、`DOI`、`URL`、`volume`、`issue`、`page`、`publisher`。`author` 是 `[{"family": "Mikolov", "given": "Tomas"}]` 或 `[{"literal": "Zotero"}]`，`issued` 是 `{"date-parts": [[2013]]}`。别的字段会让构建失败。连上 Zotero 的用户由插件的「更新博客引用数据」维护这个文件。

**系列和主题。** 系列、分组和文章顺序在 `site/data/collections.json`；主题页的层级在 `site/config.toml` 的 `topic_tree`，标签的中文名在 `tag_labels`。

**构建。** 一律用 `python scripts/build_site.py` 或 `python scripts/editor.py check`。直接运行 `zola build` 或 `zola serve` 会跳过公式保护和引用排版，结果不对。

**插件。** 改了 `extensions/research-blog/src/` 之后运行 `npm --prefix extensions/research-blog test` 和 `npm --prefix extensions/research-blog run build`。要装进 VS Code，先 `npm --prefix extensions/research-blog run package`，再从 VSIX 安装。装好的插件不读目录里的文件，所以 ZEN 改了之后要重新打包安装才会变。主题同理，目录是 `extensions/research-notes-theme`。

**文献工作台。** 它是 VS Code 里的界面，通过 `http://127.0.0.1:23119/api/` 连本机 Zotero。agent 驱动不了它，也不要绕过它去读 Zotero 的数据库。

**隐私。** `site/` 下不放密钥、PDF、Zotero 数据和联系方式。`.env`、`.tools/`、`output/`、`target/`、`site/public/`、`*.vsix` 都在 `.gitignore` 里，不要提交。`check_site.py` 检查已知的泄露规则，但不能替人审查文章内容。

## 每种改动怎么验证

| 改了什么 | 运行 |
| --- | --- |
| `ZEN.md` | `node scripts/sync_zen.cjs`，然后 `node scripts/sync_zen.cjs --check` |
| 文章、书目、系列、`config.toml` | `python scripts/editor.py check` |
| 模板、CSS、前端脚本 | `python scripts/editor.py check`，再在预览里看深浅两套主题和窄屏 |
| 插件源码 | `npm --prefix extensions/research-blog test` 和 `npm --prefix extensions/research-blog run build` |
| `crates/` | `cargo fmt --all -- --check`、`cargo test --workspace --locked`、`cargo clippy --workspace --all-targets --locked -- -D warnings` |

预览的行为用 `python scripts/editor.py preview --no-open` 验证：终端会打印地址，改文件后用 HTTP 请求确认页面更新，结束时停掉进程。

## 环境

需要 Python 3.11+、Node.js 18+、Zola 0.23.4、Typst 0.15.1 和一种中文字体。Python 脚本只用标准库。

```sh
python scripts/setup_tools.py                     # 检查，缺什么就下载到 .tools/
npm --prefix extensions/research-blog ci --ignore-scripts
npm --prefix extensions/research-blog run build   # 预览用的本地编辑服务
```

构建和预览脚本按这个顺序找 Zola 和 Typst：环境变量 `ZOLA`、`TYPST`，这个目录的 `.tools/zola/` 和 `.tools/typst/`，然后 PATH。所以 `.tools/` 里的版本会盖过系统里不对的版本。完整流程在 Windows 上验证过，发布工作流在 Linux 上构建，macOS 还没有人走过。

## 发布

GitHub Pages 的步骤：在 GitHub 新建一个空仓库，在 Settings 的 Pages 里把 Source 选成 GitHub Actions；确认 `site/config.toml` 的 `base_url` 和实际地址一致，比如 `https://USERNAME.github.io/REPO/`；然后提交并推送 `main`。工作流会安装固定版本的工具，构建、检查，再发布 `site/public/`。推送之前先运行 `python scripts/editor.py check`。

## 已知的坑

- Zola 和 Typst 的版本要精确到上面两个号，模板和 shortcode 的行为随版本变。
- Typst 第一次编译图形会联网下载 CeTZ 包，没有网络时图形编译会失败。
- Linux 需要中文字体，发布工作流装的是 `fonts-noto-cjk`。没有中文字体时 Typst 图里的中文是方块。
- `base_url` 可以带子路径，所有站内链接由它生成。
- 预览占用 1111 到 1120 之间第一个空闲端口。第一次预览之前必须先完成上面的 `npm ci` 和 `npm run build`。
- `site/content/_index.md` 的 `sort_by = "none"` 不能改成按日期，否则没有日期的页面会消失。
- 文章目录可以用中文名。路径处理一律用 `pathlib`，不要手拼字符串。
- 公式里的 `\`、`aligned`、矩阵要原样保留，不要"修正"反斜杠。
- `[[链接]]`、`[@引用]` 和公式在代码块、行内代码里都不生效，这是有意的。
- 插件的 `npm ci` 要加 `--ignore-scripts`。

## 要先问人的事

- 推送到 `main`。这会触发 GitHub Pages 部署。
- 删除或重写已有的文章、`site/data/` 里的数据，或者任何已经发布的内容。
- 改 `base_url`、站名、作者名。
- 需要密钥、账号，或者会向外部服务发送内容的事，包括 AI 补全和生图。
- 改 `ZEN.md` 的 schema、键名或两段 JSON 的标记。
- 升级 Zola、Typst、Node 这类大的依赖变更。

## 写文案的规矩

网站、README、提交说明和界面文字都用平实的中文：说清楚做什么、怎么用，短句。不用口号式的对仗句，不用「让……」「从……到……再到……」这类句式，不堆形容词。命令、路径、版本号要写准。中英对照时中文在前，英文在下。示例里不要伪造研究结果、论文或引用，演示数据要标明是演示。设计上遵守 ZEN：黑白灰为主，细线，留白，红色只点一下。
