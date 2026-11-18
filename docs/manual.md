# Research Blog 使用手册

[返回项目首页](../README.md) · [从干净模板开始](getting-started.md) · [自定义风格](style-guide.md) · [架构说明](architecture.md)

本手册保留构建、文章格式、预览、插件、个人资料和文献管理的详细操作。个人资料与简历是可选功能；初始化的新站点默认关闭。

## 本地构建与预览

静态网站不需要 LLM 密钥或后端服务。只需要 `blogctl` 和一种中文字体。`blogctl` 第一次构建或预览时会把 Zola **0.23.4** 和 Typst **0.15.1** 下载到用户缓存目录并校验哈希；`blogctl tools` 可以提前下载、查看位置。也可以用环境变量 `ZOLA`、`TYPST` 指定路径。图形首次编译会下载 CeTZ 包。Linux 需要中文字体（例如 `fonts-noto-cjk`）。

```powershell
blogctl build      # 构建到 site/public/
blogctl validate   # 检查已构建的网站
blogctl check      # 两步一起做
blogctl preview    # 实时预览，含草稿
```

访问终端打印的本地地址。预览命令会生成文章图形，再启动包含草稿的网站；启用个人资料导出时也会生成个人介绍和简历。保存源文件后自动更新。静态输出位于 `site/public/`。可用环境变量 `ZOLA`、`TYPST` 指定对应程序的完整路径。以下说明同时适用于生成的独立站点与本仓库个人站点；个人资料部分仅在开启对应功能时使用。

## 外观

默认外观参考 [Apollo](https://github.com/not-matthias/apollo) 的极简排版，随附 ZEN 明暗配色。用户可以在 [ZEN.md](../ZEN.md) 定义自己的风格，修改颜色、字体、正文尺度、基础间距、图形参数和 AI 规则；详细参数见[自定义风格指南](style-guide.md)。`site/static/zen.css` 提供生成的语义变量，`apollo.css` 与其他样式保留组件布局和细节排版，可继续定制。默认深色，明暗选择保存在浏览器中。首页扫描线和光晕均为静态背景，不覆盖文章正文；在调色板的 `effects` 中将 `halo`、`scanline` 设为 `transparent` 即关闭。

编辑 `zen:tokens` 和 `zen:prompts` 两个 JSON 块后运行 `blogctl zen`，网站构建和预览也会自动同步。`blogctl zen --check` 只检查是否遗漏同步。网站实时预览会监听 `ZEN.md`，保存后更新样式；已安装的插件需重新打包 VSIX 并安装才会更新。字体不会自动下载，配色主题也不会修改 VS Code 编辑器字体；主题名称与插件标识保持不变。不要手改生成文件。

## 写文章

安装本地新版 `extensions/research-blog/research-blog-0.3.0.vsix` 并重载 VS Code 后，可在命令面板搜索 `Research Blog`：**新建系列 → 新建文章**，需要分章节时再建组。侧栏文章右键可移动到系列或组，并上下调整阅读顺序。运行本地预览，打开“系列”页后也可直接拖动文章排序、移动归属，修改会保存到 `site/data/collections.json`。文章文件与网址不变；正式发布自动排除草稿和预览编辑入口。

选中文章内容后，右键 **AI 根据选中文字生成配图**，默认直接生成正文示意图，保存到文章 `assets/` 并插入链接。图形配色和规则来自你的风格源；在 `.typ` 文件输入 `blogdiagram` 可插入模板，Typst AI 补全沿用相同规则，也会保留作者已有的变量与布局。百炼套餐用户运行 **配置百炼 Token Plan 生图**，填写套餐 Key 即可使用 Qwen Image 3.0 Pro；文字模型可以继续使用硅基流动。其他服务运行 **配置 AI 生图接口**。详见[新版写作功能说明](../extensions/research-blog/README.md)。

主题页通过左侧多级目录切换分类，支持逐层展开收起，选择“全部”恢复完整列表。在 `site/config.toml` 的 `[extra]` 中编辑 `topic_tree`：每项可设置 `id`、`label`、`tag` 和递归的 `children`。父分类汇总子分类文章并去重；未配置的标签仍会保留。`tag_labels` 设置中文名称，`tag_descriptions` 设置分类描述。关闭 JavaScript 后仍可展开各主题并阅读文章。

```powershell
blogctl new "我的研究笔记"
```

文件生成于 `site/content/posts/我的研究笔记/index.md`，默认 `draft = true`。准备发布时改为 `false`（或移除该字段）。也可以直接创建 Markdown，不必用命令。

```toml
+++
title = "文章标题"
date = 2026-09-08
description = "一句话摘要"
draft = false
[taxonomies]
tags = ["agent", "skill-learning"]
[extra]
author = "你的名字"
+++
```

正文使用普通 Markdown；行内公式写 `$x_i^2$`，独立公式直接写：

```latex
$$
\forall s \in S: Q^*_{\mathcal{M}}(s, \tau_1) = Q^*_{\mathcal{M}}(s, \tau_2)
$$
```

不需要 HTML 包装，也不需要把 `_`、`*` 再转义。`\(...\)` 和 `\[...\]` 同样支持；矩阵或 `aligned` 中的 `\\` 保持原样。代码块、行内代码中的公式示例不会被渲染。普通价格使用 `\$` 表示字面美元符号。

`blogctl build`、`check` 和 `preview` 只在临时构建副本中保护公式，原始 Markdown 不会被改写。直接运行原生 `zola build/serve` 会绕过这一步。旧的 `math-block` 包装仍然兼容。

在文章目录的 `figures/` 下放置 `.typ` 文件，并使用 Zola 组件引用：

```text
{{ <fig id="example" caption="图标题" alt="对图内容的描述" /> }}
```

`blogctl build` 会将 `figures/example.typ` 编译为文章目录下的 `example.svg`（与 `index.md` 同级），然后生成网站。图形可通过点击或键盘放大；代码块支持复制。

## GitHub Pages

`.github/workflows/pages.yml` 在推送到 `main` 时从源码编译 `blogctl`，执行前端测试，用 `blogctl check` 编译文章图形和已启用的个人资料、构建 Zola、检查站内链接与搜索索引，再新建一个演示站点走一遍构建和 ZEN 检查，然后发布 `site/public/`。PR 只构建检查，不发布。仓库的 Settings → Pages → Source 应为 **GitHub Actions**。

日常发布：修改文章 → 提交并推送 `main` → 等待 Actions 成功。修改仓库名或自定义域名时，更新 `site/config.toml` 的 `base_url`；所有站内资源均使用该地址生成，支持 Pages 子路径。

`.env`、构建目录、课程提取材料和临时文件不会进入源码提交。文章 manifest 仅供本地 Agent 使用，静态发布脚本会从产物中移除。不要把任何密钥写入 `site/`、模板或 GitHub Pages。

## 可选文章助手

GitHub Pages 不运行 Rust 后端。公开网站默认关闭助手；搜索、阅读、公式、图形和主题归档不依赖助手。

助手不在默认的 `blogctl` 里，需要从源码编译带 `agent` 功能的版本。在本地创建 `.env`（参考 `.env.example`），然后：

```powershell
cargo run -p blogctl --features agent -- agent index
cargo run -p blogctl --features agent -- agent serve --port 3030
```

`agent index` 为已发布文章生成 `output/index/` 的中文检索索引和 manifest。`agent serve` 只启动 API，不启动网站预览，也不自动重建索引。

要启用助手，在 `site/config.toml` 的 `[extra]` 中设置 `agent_endpoint` 为完整接口 URL，例如本地使用 `http://127.0.0.1:3030/api/chat`，线上使用独立服务的 HTTPS 地址。密钥只放后端环境变量。当前服务只监听本机，适合本地或反向代理后使用；公开后端需要另行配置认证、允许的来源和限流，再开放访问。

助手支持选中文字提问、Ctrl/Cmd+Enter 发送、流式输出以及关闭时取消请求。它不具备文件写入或发布权限。

## 验证

```powershell
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
node --test tests/*.test.cjs
cargo run -p blogctl -- check
```

Rust 测试覆盖公式保护、引用排版、构建副本、ZEN 生成、站点检查、预览编辑服务、新站点初始化，以及文章助手的解析和检索；前端测试覆盖中英文搜索及任意网络分块的 SSE 解析。静态检查覆盖站内路径、目录锚点、搜索数据、RSS、站点地图和公开产物排除规则。

## 目录

| 目录 | 用途 |
| --- | --- |
| `crates/blogctl` | 命令行程序：新建站点和文章、构建、预览、检查 |
| `crates/blog-site` | 构建副本、公式保护、引用排版、ZEN、站点检查、预览、工具下载 |
| `crates/blog-ir`、`blog-build`、`blog-agent` | 可选的文章助手：文章解析、Tantivy 中文索引、Axum API |
| `site/content` | 文章源文件 |
| `profile/cv.typ` | 个人介绍与简历的统一源文件 |
| `output/pdf` | 本地简历与公开简历的生成结果 |
| `site/templates`、`site/static` | 模板、样式与交互 |
| `tests` | 前端脚本测试 |
| `extensions/research-blog` | VS Code 插件源码、LSP、AI 补全与使用文档 |

当前示例文章属于方法论笔记，没有实验结果。文章可以保存 refs.bib，或从 Zotero 工作台插入引用；构建自动排版参考文献并生成文献页。图形每次完整编译，以保证导入文件修改和缺失产物不会被缓存漏掉。

## VS Code 实时写作预览

运行 `blogctl preview`，在本地网页查看包含草稿的实际排版。可在 VS Code 配置保存后更新，或设置 `files.autoSave = "afterDelay"`；预览每 100 ms 同步源文件到包含公式保护的构建副本，Zola 监听等待为 1 ms，保持完整站点重建以同步文章列表、主题与搜索。浏览器通过仅在本地 serve 模式加载的 `preview.js` 更新正文，复用 KaTeX、保留滚动位置。重建期间的短暂 404、网络中断或格式错误会保留上一版页面并自动重试；持续失败时显示提示，可点击重试或继续保存恢复。文章 Typst 图形由常驻 watch 进程编译；开启个人资料功能时，`profile/` 中的资料、导入文件和素材在保存稳定约 0.4 秒后重新生成个人页与简历。个人资料编译失败时保留上一版产物，修正并保存后自动恢复。

`node --test tests/preview.test.cjs` 检查浏览器保留旧正文、自动恢复及重试上限。

## 双链与知识图谱

正文支持 `[[文章标题]]`、`[[文章标题|显示文字]]`、`[[文章标题#章节标题]]` 和 `[[#本文章节]]`。标题可直接使用中文；同名文章请用目录名（例如 `[[reusable-skills]]`）或内容路径（例如 `[[posts/reusable-skills/index.md]]`）区分。章节按照实际生成的目录解析，无需手写拼音锚点。

文章底部自动显示“引用了”和“被这些文章引用”；普通 Markdown 文章链接也计入关系。代码块、行内代码和公式不会产生双链；不存在、重名或章节缺失的目标显示虚线提示，不会生成错误跳转。可在 frontmatter 的 `[extra]` 内设置 `aliases = ["简称", "旧标题"]` 作为额外匹配名称；这与 Zola 顶层用于网址重定向的 `aliases` 不同。

顶栏“图谱”打开全站关系图：点击节点展开阅读入口与关联操作，悬停查看相邻节点；支持拖动节点或空白、滚轮/按钮缩放、搜索标题或主题。“文章列表”默认收起，展开后可用下拉框聚焦文章，或通过文章列表直接阅读。文章底部的图谱入口会自动聚焦当前文章。

关系由 Zola 输出的文章索引在浏览器中解析，无后端或额外 CDN 依赖。保存后同步刷新；未变化文章的解析结果和图谱布局会缓存。连线与节点依次显现，聚焦和缩放使用有限时长的过渡，没有持续动画循环；系统开启减少动态效果时停用这些动效。公开构建索引仅包含已发布文章，本地 `--drafts` 预览包含草稿。普通 Markdown 预览及 RSS 不执行这些网页脚本；需要在 RSS 中跳转时请使用标准 Markdown 链接。

本地草稿 `site/content/posts/双链与图谱使用说明/index.md` 提供可点击示例；VS Code 输入 `bloglink` 或 `blogsectionlink` 可插入双链。

## VS Code 语言插件

`extensions/research-blog` 提供此主题的 VS Code 插件：双链与章节补全、跳转和引用查找，TOML 元数据诊断，tags 补全与重复检查，以及 Typst 图形 ID 补全。配套 Tinymist 负责 `.typ` 语言服务，公式和代码可配置 AI 行内补全（默认关闭，密钥存入 VS Code SecretStorage）。

新功能使用上方源码打包的 `research-blog-0.3.0.vsix`；也可从对应 Actions 的 Artifacts 下载构建包。在 VS Code 扩展面板的菜单中选择 **Install from VSIX…** 安装，然后用 VS Code 打开站点仓库根目录。若当前窗口尚未生效，运行 **Developer: Reload Window**。公开的 `vscode-v0.1.0` Release 保留作历史版本，不代表当前功能。

输入 `[[` 补全文章、`[[文章#` 补全章节，在 `[taxonomies]` 的 `tags` 数组中按 Ctrl+Space 选择标签；F12 跳转、Shift+F12 查找引用。打开 `figures/*.typ` 使用 Tinymist。AI 补全需在用户设置中填写接口和模型，并通过 **Research Blog: 设置 AI API Key** 保存密钥；建议以灰色文字显示，Tab 接受。

完整示例、配置表、升级和排错步骤见 [插件使用说明](../extensions/research-blog/README.md)，版本变化见 [更新记录](../extensions/research-blog/CHANGELOG.md)。插件已上架 [VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=mryu888-plus.research-blog)，离线安装包也同步放在 GitHub Releases。

开发者在插件目录运行 `npm ci --ignore-scripts`、`npm run build`、`npm test` 和 `npm run package` 可生成 `.vsix`。`.github/workflows/vscode-extension.yml` 会在 Windows 和 Linux 上构建、测试并保存安装包；下载入口位于对应 Actions 运行的 Artifacts。

## 一份 Typst，同时维护个人介绍与简历

日常只编辑 `profile/cv.typ`：姓名、简介、教育经历、项目、论文及其状态都从这里生成。导航“关于”打开 `/about/`，页面中的下载入口提供同一份资料生成的公开 PDF。原有个人介绍和简历中的经历日期已保留，后续以文件里的明确日期为准，不会按构建时间自动改写。

```powershell
# 生成个人介绍数据、本地简历和公开简历
blogctl resume

# 边写边看 /about/，保存后自动更新网页和 PDF
blogctl preview

# 生成并检查完整网站
blogctl check
```

VS Code 的“终端 → 运行任务”中也有“博客：生成个人介绍与简历”“博客：预览网站（含草稿）”和“博客：构建并检查”。打开 `profile/cv.typ` 可继续使用 Tinymist 编辑和预览排版；网站预览由上面的博客任务启动。

| 产物 | 用途 |
| --- | --- |
| `output/pdf/resume.pdf` | 本地投递用简历，可包含本机的联系信息 |
| `output/pdf/resume-public.pdf` | 公开简历的本地副本 |
| `site/static/resume.pdf` | 构建时复制到网站、供访客下载的公开简历 |
| `site/data/profile.json` | 给个人页模板读取的生成数据 |

本机电话号码放在 `profile/contact.local.json`，格式为 `{"phone": "你的电话号码"}`；只支持 `phone` 字段。该文件及生成产物都由 Git 忽略，公开网页和公开 PDF 不读取本机电话。GitHub Actions 根据提交的 Typst 源文件重新生成公开产物，无需手工上传 PDF。`resume` 命令只生成资料与 PDF，完整 HTML 在预览或网站构建时生成。

`site/content/about.md` 现在只保留页面路由和模板设置，个人资料无需再编辑这份 Markdown，也不要手改生成的 JSON 或 PDF。首页署名同样从 Typst 读取；页面样式在 `site/static/about.css`，公共外观由 `apollo.css` 管理。实时预览输出到 `.tools/site-preview`，不会把草稿混入 `site/public` 的发布产物。

`blogctl build` 在运行 Zola 前生成个人资料。字体优先使用 Noto CJK，本机没有安装时自动使用微软雅黑／宋体；CI 安装 Noto CJK。`layout.typ` 只负责排版，平时无需修改。

配图规则统一维护在 [ZEN.md](../ZEN.md)，可编辑的精确图形模板在 `design/visual-system/typst/blog-diagram.typ`。历史配图探索素材与个人研究图片不进入干净站点模板。

## 文献管理与阅读笔记

安装 `extensions/research-blog/research-blog-0.3.0.vsix` 后，命令面板运行 **Research Blog: 打开文献工作台**，或打开资源管理器里的“文献库”。工作台支持分类与标签筛选、标题/作者/年份检索、查看 PDF 批注、打开原文、插入引用和批注生成阅读草稿。

PDF 阅读、绘制与修改批注、附件存储和跨设备同步由 Zotero 提供。已有 Zotero 用户先打开客户端，在 **设置 → 高级** 开启“允许本机其他程序与 Zotero 通信”。需要在工作台修改文献、导入 PDF 或创建分类时，点击“授权修改”，在 Zotero 的授权窗口选择 **Always Allow**；密钥仅保存在 VS Code SecretStorage。Zotero 10 及以上支持这些本地写入；旧版可只读浏览、引用和导出批注。

Windows 没有安装 Zotero 时，可以运行 **Research Blog: 启动本地 Zotero**，或使用：

```powershell
blogctl zotero setup
blogctl zotero start
```

助手下载官方 Zotero 10.0.6 便携版，客户端、独立配置和文献库存放于 Git 忽略的 `.tools/zotero/`，不会进入网站。已有系统 Zotero 仍可直接连接；若要切换到本项目独立库，应先退出其他 Zotero 实例。非 Windows 用户安装官方桌面版后使用同一工作台。

跨设备同步在 Zotero **设置 → 同步** 中配置自己的账号；文献数据和 PDF 附件分开同步，附件可使用 Zotero Storage 或个人库 WebDAV。博客文章及公开书目快照随 Git 管理。便携版的 `.tools/` 文献库需要 Zotero 同步或备份；它不随仓库提交。

在文章正文运行 **插入文献引用**，插件插入 `[@zotero-XXXXXXXX]`，同时保存该文献的公开书目快照到 `site/data/references.json`。可在 Zotero Extra 中设置 `Citation Key: yourkey`，已有 Better BibTeX 引用键也可使用；无需安装 Better BibTeX 才能引用。输入 `[@` 可补全，悬停显示书目信息，F12 打开书目来源，Shift+F12 查找文章引用。

```markdown
单篇引用 [@yourkey]。
多个引用 [@yourkey; @anotherkey]。
引用指定页码 [@yourkey, p. 3]。
```

**从批注创建阅读笔记** 会生成 `draft = true` 的文章，保留批注文字、评论、页码和跳转 Zotero 原文的链接。检查笔记并补充自己的分析后再发布。插入引用和创建笔记自动更新书目快照；批量同步文献信息可运行 **更新博客引用数据**。

构建离线读取快照和文章目录的 `refs.bib`，在正文中显示数字编号引用、文章末尾生成参考文献，并在 `/references/` 汇总已发布文章实际引用的文献和反向链接。支持搜索文献；未被公开文章引用的条目不展示。代码、公式、转义文本里的引用示例不会被识别。缺失书目会给出文件位置与引用键，修正后重新构建即可。BibTeX 支持标准条目、姓名、字符串宏及 crossref；复杂 LaTeX 字段建议使用 Zotero 导出的公开 JSON 快照。

私有 PDF、原始库、API 密钥和本机路径不写入 `site/`。文献工作台的离线缓存位于 VS Code 私有扩展存储。离线时可以浏览、引用已缓存的文献，修改与 PDF 导入需连接 Zotero。正式网站只发布选定引用的书目信息和你主动发布的笔记。

| 配置 | 默认值 | 用途 |
| --- | --- | --- |
| `researchBlog.zotero.baseUrl` | `http://127.0.0.1:23119/api/` | 本机接口，支持 localhost / IPv6 loopback |
| `researchBlog.zotero.libraryPath` | `users/0` | 个人库；群组库使用 `groups/数字ID` |
| `researchBlog.zotero.autoRefreshSeconds` | `60` | 打开工作台后的刷新间隔；0 关闭 |

验证：`npm --prefix extensions/research-blog test`、`cargo test -p blog-site`、`blogctl check`。本地运行与 API 协议见 [Zotero Local API](https://www.zotero.org/support/dev/web_api/v3/local_api)，同步设置见 [Zotero Sync](https://www.zotero.org/support/sync)。
