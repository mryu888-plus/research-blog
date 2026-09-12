# Research Blog

一个 Rust + Zola 驱动的中文研究博客，包含公式排版、Typst 图形、主题归档、全文搜索及可选的文章助手。

- 网站：https://mryu888-plus.github.io/research-blog/
- 源码：https://github.com/mryu888-plus/research-blog

## 本地构建与预览

静态网站不需要 LLM 密钥或 Rust 服务。需要 Python 3.11+、Zola **0.23.4**、Typst **0.15.1**。下载官方二进制并加入 PATH；Windows 也可以将 Zola 放在 `.tools/zola/zola.exe`。图形首次编译会下载 CeTZ 包。Linux 需要中文字体（例如 `fonts-noto-cjk`）。

```powershell
python scripts/build_site.py
python scripts/check_site.py
zola --root site serve --interface 127.0.0.1 --port 1111
```

访问终端打印的本地地址。Windows 项目内安装的 Zola 可用 `.tools/zola/zola.exe` 替代 `zola`。静态输出位于 `site/public/`。

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

`python scripts/build_site.py` 会将 `figures/example.typ` 编译为同目录的 `example.svg`，然后生成网站。图形可通过点击或键盘放大；代码块支持复制。

## GitHub Pages

`.github/workflows/pages.yml` 在推送到 `main` 时执行前端测试、编译图形、构建 Zola、检查站内链接与搜索索引，然后发布 `site/public/`。PR 只构建检查，不发布。仓库的 Settings → Pages → Source 应为 **GitHub Actions**。

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
| `site/templates`、`site/static` | 模板、样式与交互 |
| `scripts`、`tests` | 静态构建与验证 |

当前示例文章属于方法论笔记，没有实验结果。BibTeX 文件可随文章保存，但尚未实现自动参考文献排版。图形每次完整编译，以保证导入文件修改和缺失产物不会被缓存漏掉。

## VS Code 实时写作预览

运行 `python scripts/editor.py preview`，在本地网页查看包含草稿的实际排版。项目 VS Code 配置使用 25 ms 自动保存；Zola 监听等待缩短到 1 ms，保持完整站点重建以同步文章列表、主题与搜索。浏览器通过仅在本地 serve 模式加载的 `preview.js` 更新正文，复用 KaTeX、保留滚动位置。重建期间的短暂 404、网络中断或格式错误会保留上一版页面并自动重试；持续失败时显示提示，可点击重试或继续保存恢复。Typst 由常驻 watch 进程编译。

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

安装、AI 配置和开发步骤见 [插件说明](extensions/research-blog/README.md)。在插件目录运行 `npm ci`、`npm run build`、`npm test` 和 `npm run package` 可生成 `.vsix`；通过 VS Code 的 **Install from VSIX…** 安装。
