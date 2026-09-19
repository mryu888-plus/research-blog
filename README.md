# Research Blog

一个 Rust + Zola 驱动的中文研究博客，包含公式排版、Typst 图形、主题归档、全文搜索及可选的文章助手。

- 网站：https://mryu888-plus.github.io/research-blog/
- 源码：https://github.com/mryu888-plus/research-blog
- VS Code 插件：[下载 v0.1.0 安装包](https://github.com/mryu888-plus/research-blog/releases/download/vscode-v0.1.0/research-blog-0.1.0.vsix) · [使用说明](extensions/research-blog/README.md)

## 本地构建与预览

静态网站不需要 LLM 密钥或 Rust 服务。需要 Python 3.11+、Zola **0.23.4**、Typst **0.15.1**。下载官方二进制并加入 PATH；Windows 也可以将 Zola 放在 `.tools/zola/zola.exe`。图形首次编译会下载 CeTZ 包。Linux 需要中文字体（例如 `fonts-noto-cjk`）。

```powershell
python scripts/build_site.py
python scripts/check_site.py
python scripts/editor.py preview
```

访问终端打印的本地地址。预览命令会先生成个人介绍、简历和文章图形，再启动包含草稿的网站；保存源文件后自动更新。静态输出位于 `site/public/`。可用环境变量 `ZOLA`、`TYPST` 指定对应程序的完整路径。

## 外观

当前外观参考 [Apollo](https://github.com/not-matthias/apollo) 的极简排版，在现有模板上定制，保留搜索、双链、图谱和实时预览。样式集中在 `site/static/apollo.css`，加载于原有基础样式之后；默认深色，明暗选择保存在浏览器中。首页扫描线和光晕均为静态背景，不覆盖文章正文。调整 `--halo`、`--scanline` 可以控制效果强度，设为 `transparent` 即关闭。

## 写文章

主题页通过左侧多级目录切换分类，支持逐层展开收起，选择“全部”恢复完整列表。在 `site/config.toml` 的 `[extra]` 中编辑 `topic_tree`：每项可设置 `id`、`label`、`tag` 和递归的 `children`。父分类汇总子分类文章并去重；未配置的标签仍会保留。`tag_labels` 设置中文名称，`tag_descriptions` 设置分类描述。关闭 JavaScript 后仍可展开各主题并阅读文章。

```powershell
cargo run -p blogctl -- new my-research-note
```

文件生成于 `site/content/posts/my-research-note/index.md`，默认 `draft = true`。准备发布时改为 `false`（或移除该字段）。也可以直接创建 Markdown，不必使用 Rust CLI。

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

正文使用 Markdown；公式支持 `$x^2$` 和 `$$ ... $$`。

包含下划线、星号或转义括号的独立公式，请在 `$$` 块外包一层 `<div class="math-block">` 和 `</div>`（各占一行，内部不留空行），防止 Markdown 在公式渲染前改变 TeX。现有示例文章包含完整写法。

在文章目录的 `figures/` 下放置 `.typ` 文件，并使用 Zola 组件引用：

```text
{{ <fig id="example" caption="图标题" alt="对图内容的描述" /> }}
```

`python scripts/build_site.py` 会将 `figures/example.typ` 编译为文章目录下的 `example.svg`（与 `index.md` 同级），然后生成网站。图形可通过点击或键盘放大；代码块支持复制。

## GitHub Pages

`.github/workflows/pages.yml` 在推送到 `main` 时执行前端与个人资料导出测试，编译个人介绍、公开简历和文章图形，构建 Zola，检查站内链接与搜索索引，然后发布 `site/public/`。PR 只构建检查，不发布。仓库的 Settings → Pages → Source 应为 **GitHub Actions**。

日常发布：修改文章 → 提交并推送 `main` → 等待 Actions 成功。修改仓库名或自定义域名时，更新 `site/config.toml` 的 `base_url`；所有站内资源均使用该地址生成，支持 Pages 子路径。

`.env`、构建目录、课程提取材料和临时文件不会进入源码提交。文章 manifest 仅供本地 Agent 使用，静态发布脚本会从产物中移除。不要把任何密钥写入 `site/`、模板或 GitHub Pages。

## 可选文章助手

GitHub Pages 不运行 Rust 后端。公开网站默认关闭助手；搜索、阅读、公式、图形和主题归档不依赖助手。

在本地创建 `.env`（参考 `.env.example`），然后：

```powershell
cargo run -p blogctl -- build
cargo run -p blogctl -- serve --port 3030
```

Rust 完整构建同时生成 `output/index/` 的中文检索索引和 manifest。`serve` 只启动 API，不启动网站预览，也不自动重建索引。构建需要 Zola 和 Typst 位于 PATH。

要启用助手，在 `site/config.toml` 的 `[extra]` 中设置 `agent_endpoint` 为完整接口 URL，例如本地使用 `http://127.0.0.1:3030/api/chat`，线上使用独立服务的 HTTPS 地址。密钥只放后端环境变量。当前服务只监听本机，适合本地或反向代理后使用；公开后端需要另行配置认证、允许的来源和限流，再开放访问。

助手支持选中文字提问、Ctrl/Cmd+Enter 发送、流式输出以及关闭时取消请求。它不具备文件写入或发布权限。

## 验证

```powershell
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
node --test tests/*.test.cjs
python -m unittest discover -s tests -p 'test_profile.py'
python scripts/build_site.py
python scripts/check_site.py
```

Rust 测试覆盖文章解析、中文检索、元数据一致性及文章路径校验；前端测试覆盖中英文搜索及任意网络分块的 SSE 解析。静态检查覆盖站内路径、目录锚点、搜索数据、RSS、站点地图和公开产物排除规则。

## 目录

| 目录 | 用途 |
| --- | --- |
| `crates/blog-ir` | Markdown 与 TOML 解析、文章中间表示 |
| `crates/blog-build` | Typst、Zola 构建和 Tantivy 中文索引 |
| `crates/blog-agent` | Axum API、检索与 LLM 流式调用 |
| `crates/blogctl` | 创建文章、构建、索引和启动 API |
| `site/content` | 文章源文件 |
| `profile/cv.typ` | 个人介绍与简历的统一源文件 |
| `output/pdf` | 本地简历与公开简历的生成结果 |
| `site/templates`、`site/static` | 模板、样式与交互 |
| `scripts`、`tests` | 静态构建与验证 |
| `extensions/research-blog` | VS Code 插件源码、LSP、AI 补全与使用文档 |

当前示例文章属于方法论笔记，没有实验结果。BibTeX 文件可随文章保存，但尚未实现自动参考文献排版。图形每次完整编译，以保证导入文件修改和缺失产物不会被缓存漏掉。

## VS Code 实时写作预览

运行 `python scripts/editor.py preview`，在本地网页查看包含草稿的实际排版。项目 VS Code 配置使用 25 ms 自动保存；Zola 监听等待缩短到 1 ms，保持完整站点重建以同步文章列表、主题与搜索。浏览器通过仅在本地 serve 模式加载的 `preview.js` 更新正文，复用 KaTeX、保留滚动位置。重建期间的短暂 404、网络中断或格式错误会保留上一版页面并自动重试；持续失败时显示提示，可点击重试或继续保存恢复。文章 Typst 图形由常驻 watch 进程编译；`profile/` 中的个人资料、导入文件和素材在保存稳定约 0.4 秒后重新生成个人页与简历。个人资料编译失败时保留上一版产物，修正并保存后自动恢复。

性能复测：`python scripts/benchmark_preview.py` 在项目临时目录复制真实网站，测量写文件到新 HTML 可读取的耗时，不修改文章。2026-09-12 的 20 次样本：监听等待 200 ms 时中位数 261.51 ms，等待 1 ms 时 47.67 ms、P95 50.01 ms。浏览器实测正文和公式局部更新约 2.6–3.5 ms；这些是分段测量，不代表完整按键到屏幕延迟。

`python scripts/test_preview_rebuild.py` 可在隔离副本中复现 Zola 重建期间的 HTTP 404，确认重建后恢复 200；`node --test tests/preview.test.cjs` 检查浏览器保留旧正文、自动恢复及重试上限。

## 双链与知识图谱

正文支持 `[[文章标题]]`、`[[文章标题|显示文字]]`、`[[文章标题#章节标题]]` 和 `[[#本文章节]]`。标题可直接使用中文；同名文章请用目录名（例如 `[[reusable-skills]]`）或内容路径（例如 `[[posts/reusable-skills/index.md]]`）区分。章节按照实际生成的目录解析，无需手写拼音锚点。

文章底部自动显示“引用了”和“被这些文章引用”；普通 Markdown 文章链接也计入关系。代码块、行内代码和公式不会产生双链；不存在、重名或章节缺失的目标显示虚线提示，不会生成错误跳转。可在 frontmatter 的 `[extra]` 内设置 `aliases = ["简称", "旧标题"]` 作为额外匹配名称；这与 Zola 顶层用于网址重定向的 `aliases` 不同。

顶栏“图谱”打开全站关系图：点击节点展开阅读入口与关联操作，悬停查看相邻节点；支持拖动节点或空白、滚轮/按钮缩放、搜索标题或主题。“文章列表”默认收起，展开后可用下拉框聚焦文章，或通过文章列表直接阅读。文章底部的图谱入口会自动聚焦当前文章。

关系由 Zola 输出的文章索引在浏览器中解析，无后端或额外 CDN 依赖。保存后同步刷新；未变化文章的解析结果和图谱布局会缓存。连线与节点依次显现，聚焦和缩放使用有限时长的过渡，没有持续动画循环；系统开启减少动态效果时停用这些动效。公开构建索引仅包含已发布文章，本地 `--drafts` 预览包含草稿。普通 Markdown 预览及 RSS 不执行这些网页脚本；需要在 RSS 中跳转时请使用标准 Markdown 链接。

本地草稿 `site/content/posts/双链与图谱使用说明/index.md` 提供可点击示例；VS Code 输入 `bloglink` 或 `blogsectionlink` 可插入双链。

## VS Code 语言插件

`extensions/research-blog` 提供此主题的 VS Code 插件：双链与章节补全、跳转和引用查找，TOML 元数据诊断，tags 补全与重复检查，以及 Typst 图形 ID 补全。配套 Tinymist 负责 `.typ` 语言服务，公式和代码可配置 AI 行内补全（默认关闭，密钥存入 VS Code SecretStorage）。

从 [GitHub Release](https://github.com/mryu888-plus/research-blog/releases/tag/vscode-v0.1.0) 下载 `research-blog-0.1.0.vsix`，在 VS Code 扩展面板的菜单中选择 **Install from VSIX…** 安装，然后用 VS Code 打开博客仓库根目录。若当前窗口尚未生效，运行 **Developer: Reload Window**。

输入 `[[` 补全文章、`[[文章#` 补全章节，在 `[taxonomies]` 的 `tags` 数组中按 Ctrl+Space 选择标签；F12 跳转、Shift+F12 查找引用。打开 `figures/*.typ` 使用 Tinymist。AI 补全需在用户设置中填写接口和模型，并通过 **Research Blog: 设置 AI API Key** 保存密钥；建议以灰色文字显示，Tab 接受。

完整示例、配置表、升级和排错步骤见 [插件使用说明](extensions/research-blog/README.md)，版本变化见 [更新记录](extensions/research-blog/CHANGELOG.md)。源码随仓库管理，安装包通过 GitHub Releases 分发；尚未发布到 VS Code Marketplace。

开发者在插件目录运行 `npm ci --ignore-scripts`、`npm run build`、`npm test` 和 `npm run package` 可生成 `.vsix`。`.github/workflows/vscode-extension.yml` 会在 Windows 和 Linux 上构建、测试并保存安装包；下载入口位于对应 Actions 运行的 Artifacts。

## 一份 Typst，同时维护个人介绍与简历

日常只编辑 `profile/cv.typ`：姓名、简介、教育经历、项目、论文及其状态都从这里生成。导航“关于”打开 `/about/`，页面中的下载入口提供同一份资料生成的公开 PDF。原有个人介绍和简历中的经历日期已保留，后续以文件里的明确日期为准，不会按构建时间自动改写。

```powershell
# 生成个人介绍数据、本地简历和公开简历
python scripts/editor.py resume

# 边写边看 /about/，保存后自动更新网页和 PDF
python scripts/editor.py preview

# 生成并检查完整网站
python scripts/editor.py check
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

`cargo run -p blogctl -- build` 也会在运行 Zola 前调用同一份导出脚本，因此 Rust 完整构建还需要 Python 3.11+；可用 `PYTHON` 环境变量指定解释器。字体优先使用 Noto CJK，本机没有安装时自动使用微软雅黑／宋体；CI 安装 Noto CJK。`layout.typ` 只负责排版，平时无需修改。
