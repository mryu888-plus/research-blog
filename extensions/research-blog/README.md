# Research Blog Language Tools

面向 [Research Blog 主题](https://github.com/mryu888-plus/research-blog)的 VS Code 写作插件：系列和组、统一风格配图、双链与 tags、元数据检查、Typst 图形，以及可选的公式和代码 AI 补全。

## 系列、组与 AI 配图（v0.2.4）

安装本地构建的 `research-blog-0.2.4.vsix` 后重载窗口。此版本尚未发布到 GitHub Release；下方 v0.1.0 下载链接是旧版语言工具，不含这些新功能。

按 `Ctrl+Shift+P` 搜索 `Research Blog`，可直接运行 **新建系列**、**新建组**、**新建文章**、**预览网站（含草稿）**、**构建并检查**。资源管理器中也会出现 **研究手记** 侧栏。

1. **新建系列**：填写名称和可选简介，可直接放文章。
2. **新建文章**：选择系列、可选分组，或暂不加入系列。系列和组名旁的“新建文章”会直接使用该位置。
3. **新建组**：需要细分章节时再创建，分组不是添加文章的前提。
4. **整理已有文章**：打开文章后运行“移动文章到系列或组”，或使用文章的右键菜单。选择“暂不加入系列”可移出目录。
5. **调整顺序**：右键系列或组内的文章，选择“文章上移／文章下移”。文章底部目录和前后篇使用所在列表的顺序。

运行 **预览网站（含草稿）** 后打开“系列”页。拖动文章左边的 `⋮⋮`，可在列表中调整顺序，也可移入其他系列、分组或“未加入系列”。空系列同样支持投放。手机或键盘可使用每篇文章的“移动”菜单与上移／下移按钮。修改自动保存，失败时会提示；若目录被其他窗口修改，会刷新并要求重试，不覆盖新内容。正式发布的网站不提供编辑入口。

关系保存在 `site/data/collections.json`（与配置的 contentPath 同级的 `data/collections.json`）。系列用 `articles` 数组保存直属文章，`groups` 保存可选分组；旧目录可继续使用。移动只改变目录归属与顺序，不移动文章文件或改变网址。每篇文章最多出现在一个系列或组中，tags 可独立跨主题使用。名称、简介或系列/组顺序可用“编辑系列目录”修改，保留稳定 `id` 即可。

正式构建过滤草稿和已删除的文章引用；本地预览显示并标记草稿。旧的“博客：新建文章”任务创建的文章默认未加入系列。

### 按选中文字生成配图

使用百炼套餐时，运行 **Research Blog: 配置百炼 Token Plan 生图**，输入以 `sk-sp-` 开头的套餐专属 Key。插件预设 `qwen-image-3.0-pro`、1536×1024 横图（1K 档），通过 [Token Plan 专用多模态接口](https://help.aliyun.com/zh/model-studio/token-plan-multimodal-gen)调用；文字模型与密钥保持原配置，可继续使用硅基流动 GLM 整理配图构思。生图失败或套餐用尽时不切换到其他收费接口。实际 Credits 消耗以百炼套餐用量记录为准。

其他服务运行 **Research Blog: 配置 AI 生图接口**，填写基础地址、图像模型 ID 和 API Key。支持硅基流动原生生图格式或 [OpenAI Images API 的 `/images/generations`](https://developers.openai.com/api/docs/guides/image-generation)。保存配置不会请求生图；仅手动执行生图命令时发起生成。

打开文章，选中一段文字，右键 → **AI 根据选中文字生成配图**。默认使用黑白实验出版物风格：石墨黑底、象牙白细线、抽象几何与少量陶土红。运行 **Research Blog: 选择配图用途与底色**，可切换正文配图、概念图、系列封面，以及石墨黑 / 暖纸白。百炼生图默认携带内置风格参考图（会计入输入图像用量），可关闭 `researchBlog.images.useStyleReference`。仅发送选中的文字（最多 12000 字符）和固定风格说明，不上传整篇文章。风格由提示词约束，模型输出仍可能存在差异。

图片以唯一文件名保存到文章的 `assets/` 下，并在选中文段后插入 Markdown 链接、保存文章。选中代码或块级公式时，会放在闭合代码／公式块后。如果生成期间文章改变，或代码／公式块未闭合，则仅保存图片，提供“复制图片链接”和“查看图片”。取消和失败不会插入半成品。

API Key 保存在 SecretStorage。未设置生图专用 Key 时，仅当基础地址与 AI 补全接口完全一致才复用其 Key；下载接口返回的图片 URL 时不携带 Key。生图无需开启 AI 行内补全。

| 设置 | 默认值 | 用途 |
| --- | --- | --- |
| `researchBlog.images.appearance` | `graphite` | 石墨黑 / 暖纸白（`paper`） |
| `researchBlog.images.composition` | `editorial` | 正文 / 概念（`concept`）/ 封面（`cover`） |
| `researchBlog.images.useStyleReference` | `true` | 百炼生图携带内置参考图 |
| `researchBlog.images.baseUrl` | 空 | 复用 AI 地址，也可单独设置 |
| `researchBlog.images.model` | 空 | 显式填写图像模型 ID |
| `researchBlog.images.size` | `1536x1024` | 横向；不支持时选 `auto` |
| `researchBlog.images.quality` | `auto` | 服务默认质量 |
| `researchBlog.images.timeoutSeconds` | `180` | 超时，支持 30–600 秒；请求可取消 |
| `researchBlog.images.styleInstructions` | 空 | 使用统一博客风格；可在工作区中统一自定义 |

写入和 AI 功能需要受信任的本地工作区。预览与构建调用 `scripts/editor.py`，需要完整博客仓库和 Python、Zola、Typst、Node.js 环境。首次从源码使用预览时，先执行 `npm --prefix extensions/research-blog ci` 和 `npm --prefix extensions/research-blog run build`，生成本地编辑服务。

## 下载、安装和升级

需要 **VS Code 1.85.0 或更新版本**。

1. [下载 research-blog-0.1.0.vsix](https://github.com/mryu888-plus/research-blog/releases/download/vscode-v0.1.0/research-blog-0.1.0.vsix)，或打开 [v0.1.0 发布页](https://github.com/mryu888-plus/research-blog/releases/tag/vscode-v0.1.0)。安装使用发布包即可，无需编译源码。
2. 在 VS Code 按 `Ctrl+Shift+X`，点击扩展面板右上角 `…` → **Install from VSIX… / 从 VSIX 安装…**，选择下载的文件。
3. 确认已安装 **Research Blog Language Tools**（`mryu888-plus.research-blog`）和配套 **Tinymist**（`myriad-dreamin.tinymist`）。按提示重载窗口；也可在命令面板运行 **Developer: Reload Window**。

安装包将 Tinymist 声明为配套扩展。若它没有自动安装，在扩展面板搜索上述 ID 安装；离线环境需要另外准备 Tinymist 的 VSIX。博客语言服务本身不需要额外安装 Node.js、Python、Zola 或 Rust；构建网站和预览的依赖见[仓库使用文档](../../README.md)。

也可以在 PowerShell 指定下载文件的实际路径：

```powershell
code --install-extension "$env:USERPROFILE\Downloads\research-blog-0.1.0.vsix"
code --list-extensions --show-versions
```

升级时，从 [Releases](https://github.com/mryu888-plus/research-blog/releases) 下载新版本 VSIX，再执行同样的安装步骤并重载窗口。同版本重新安装可在上述安装命令末尾加 `--force`。插件尚未发布到 Marketplace，不会通过 Marketplace 自动获得新版本；升级通常会保留现有设置和 SecretStorage 中的 Key。版本变化见[更新记录](CHANGELOG.md)。

## 打开正确的目录

选择 **File → Open Folder… / 文件 → 打开文件夹…**，打开包含 `site/`、`scripts/` 和 `extensions/` 的博客仓库根目录。默认索引范围为 `site/content/**/*.md`；双链目标来自其中 `posts/` 下的文章。

如果直接打开的是 `site` 目录，在该工作区的 `.vscode/settings.json` 中设置：

```json
{
  "researchBlog.contentPath": "content"
}
```

修改后运行 **Research Blog: 重启语言服务**。`contentPath` 必须位于当前工作区内；标签中文名从文章目录上一层的 `config.toml` 中读取，例如默认位置 `site/config.toml`。多根工作区分别启动独立服务器，双链和标签索引不跨根目录合并。仅打开单个文件、未打开文件夹时，不会建立博客索引。

## 5 分钟试用

安装完成后，在博客仓库中按下面步骤创建一篇草稿。示例文件均位于 `site/content/posts/plugin-demo/`，也可以在现有文章中试用相同操作。

**1. 新建文章，试用 tags。** 创建 `index.md` 并保存：

```markdown
+++
title = "插件试写"
date = 2026-09-12
description = "试用双链、标签和 Typst 图形"
draft = true

[taxonomies]
tags = ["reinforcement-learning"]

[extra]
author = "Researcher"
aliases = ["试写"]
+++

## 问题与背景

这里记录研究问题。

## 方法

回看 [[#问题与背景]]。

{{ <fig id="example" caption="示例公式" alt="平方公式" /> }}
```

将光标放进标签字符串，按 `Ctrl+Space`：应出现已有标签，以及 `site/config.toml` 中 `[extra.tag_labels]` 配置的标签，详情包含中文名和使用文章数。也可以在已有标签后输入逗号，按 `Ctrl+Space` 补充第二个标签。标签可自行新建，并不要求先在配置文件注册；标签使用数包含索引中的草稿。

**2. 试用双链和诊断。** 在正文输入 `[[`，按 `Ctrl+Space` 选择文章；输入 `[[#` 可选本文章节，输入 `[[试写#` 可选别名对应文章的章节。在 `[[#问题与背景]]` 内按 `F12` 应跳到该标题；悬停查看目标摘要，按 `Shift+F12` 查引用。临时输入 `[[不存在的文章]]`，在 **Problems / 问题** 面板查看警告，再删除该链接。

**3. 试用 Typst。** 新建 `figures/example.typ` 并保存：

```typst
#set page(width: auto, height: auto, margin: 12pt)
#let square(x) = x * x

$ f(x) = x^2 $

#square(3)
```

回到 Markdown，将光标放在 `id="example"` 的 `example` 上按 `F12`，应打开该 `.typ` 文件，之前的“找不到图形源文件”警告应消失。在 `#square(3)` 的 `square` 上按 `F12`，由 Tinymist 跳到定义；在 `.typ` 文件中使用 Tinymist 预览。编辑图形引用时，可在同一行的 `id="` 后按 `Ctrl+Space` 选择已有图形。

**4. 可选：试用 AI。** 按下一节配置接口、模型和 Key，然后在文章正文中输入一个公式块，把光标放在 `\frac{` 后运行 **Research Blog: AI 补全公式或代码**：

```latex
$$
f(x) = \frac{
$$
```

上面展示的是要插入 Markdown 正文的内容，实际试用时不要保留外层代码围栏。也可在 Markdown 的围栏代码块里输入 `def square(x):` 后换行，或在刚才的 `.typ` 文件中请求补全。出现灰色建议后，按 **Tab 接受、Esc 忽略**。未配置 AI 时，前面三步照常可用。

## 写作功能和范围

| 输入或操作 | 效果 |
| --- | --- |
| `[[` 后输入标题、别名，或按 `Ctrl+Space` | 补全文章；同名文章用内容路径区分 |
| `[[文章#` 或 `[[#` | 补全目标文章或本文的章节标题 |
| 在双链上按 `F12`、悬停、`Shift+F12` | 跳转、摘要、查找引用 |
| 在文章 `title` 或章节标题上按 `Shift+F12` | 查找引用该文章或章节的双链 |
| frontmatter 字段处按 `Ctrl+Space` | 按当前表补全 `title`、`date`、`draft`、`description`、`tags`、`author`、`aliases` 等字段 |
| `[taxonomies]` 的 `tags = ["…"]` | 补全已有标签与 `extra.tag_labels` 的键，展示中文名和使用文章数；支持多行数组 |
| 标签处悬停或 `Shift+F12` | 查看使用次数、文章列表、标签出现位置 |
| `{{ <fig id="…" /> }}` | 补全当前文章目录的 `figures/*.typ`；`F12` 打开源文件 |
| **Outline / 大纲** | 展示文章章节 |
| **Problems / 问题** | TOML 语法、主要元数据类型、重复标签、失效或重名双链、缺失章节、公开文章引用草稿、缺失图形 |
| 输入 `blogpost`、`bloglink`、`blogsectionlink`、`blogfig` 后按 `Ctrl+Space` | 插入文章、双链、章节双链或图形模板 |

补全会保留双链显示文字和已有闭合符号。链接解析复用网站 `knowledge-core.js`，跳转可读取已打开文件尚未保存的修改，创建、修改、删除文章后索引自动更新。代码块、行内代码、注释和已闭合公式中的双括号不报链接错误。

章节建议优先使用可读标题；支持显式 `{#id}` 和常规英文锚点。Zola 自动音译的中文 URL 锚点暂不解析，请写 `[[文章#中文章节标题]]`。双链目标与主题图谱一致，来自 `posts/` 页面，不包括 `_index.md` 等 section 索引。普通 Markdown 链接由 VS Code 内置服务处理。本插件不提供重命名、自动改写链接或完整 Zola 配置校验。

标签补全和引用定位面向标准的 `[taxonomies]` 表及 `tags = [...]` 写法。`[extra]` 中的 `aliases` 是主题双链别名；请按示例放置，不要与顶层字段混用。

## Typst 与网站预览

打开文章 `figures/*.typ`，Tinymist 提供 Typst 补全、诊断、跳转、悬停、格式化和预览。命令 **Research Blog: 打开 Tinymist Typst 语言支持** 打开其扩展详情。Tinymist 的语义分析作用于 `.typ` 文件；Markdown 内嵌公式使用 LaTeX，可使用 AI 补全，但不会获得 Tinymist 的语义诊断。

本插件负责编辑辅助；预览完整网站时，在博客根目录运行：

```powershell
python scripts/editor.py preview
```

该命令需要仓库文档列出的 Python、Zola、Typst 等依赖。它会预览包含草稿的页面并监听图形变化。正式构建使用 `python scripts/build_site.py`，将 `figures/example.typ` 编译为文章目录下的 `example.svg`（与 `index.md` 同级）后生成网站。

## 可选 AI 补全

AI 默认关闭。在命令面板打开 **Preferences: Open User Settings (JSON)**，合并以下设置，并将模型替换为你的接口实际支持的 ID：

```json
{
  "researchBlog.ai.enabled": true,
  "researchBlog.ai.baseUrl": "https://api.openai.com/v1",
  "researchBlog.ai.model": "你的模型ID",
  "researchBlog.ai.tokenParameter": "max_completion_tokens",
  "editor.inlineSuggest.enabled": true
}
```

随后运行 **Research Blog: 设置 AI API Key**。Key 按接口地址保存在 VS Code SecretStorage，可用 **Research Blog: 删除 AI API Key** 删除当前接口的 Key。不要把 Key 写入仓库或 `settings.json`。修改接口地址后，需要为新接口重新设置 Key。

支持 **OpenAI Chat Completions 兼容接口**，基础地址不包含 `/chat/completions`，例如 `https://api.openai.com/v1`；插件会自动追加该路径。接口地址不能附带账号、密码、查询参数或片段。若提供商只支持旧参数 `max_tokens`，修改 `researchBlog.ai.tokenParameter`。本地兼容服务可设为 `http://127.0.0.1:端口/v1`，本机服务可不设置 Key；远程接口需要 HTTPS 和 Key。地址、模型和 token 参数须在用户设置中配置。

可触发位置包括 Markdown 的 `$…$`、`$$…$$`、`\(…\)`、`\[…\]` 内部、围栏代码块内部，以及 `.typ` 文件。文件必须是当前工作区 `contentPath` 范围内的本地文件，支持尚未保存的修改；新建的无路径 Untitled 文档不触发。普通正文、独立 `.py` / `.rs` 等代码文件不触发本插件的 AI。默认停顿 600 ms 后请求灰色建议，也可运行 **Research Blog: AI 补全公式或代码** 手动触发；手动触发仍需启用 AI。

每次默认发送当前文件光标附近最多 **4,000 个字符**，含前文和后文，也可能包含代码或公式附近的正文。请求还包含内容类型提示，最多请求 **256 个输出 token**。插件不会主动读取并发送其他文章或整个仓库，也不额外附加文件路径。AI 日志不记录正文、Key 或接口响应正文。接受建议前不会写入文档或执行代码；继续输入会取消旧请求，请求流程最多等待 15 秒。受限工作区不会请求 AI。接口按服务商规则计费，可关闭 `researchBlog.ai.enabled` 停用。

AI 请求流程已通过本机 HTTP 模拟接口验证，尚未完成真实模型端到端验证；建议质量和延迟取决于所配服务。实际写作中需自行检查生成的公式和代码。

## 全部配置

硅基流动可同时用于文字补全和生图。在用户设置中将 `researchBlog.ai.baseUrl` 与 `researchBlog.images.baseUrl` 设为 `https://api.siliconflow.cn/v1`，文字模型可用 `zai-org/GLM-5.3`，`researchBlog.ai.tokenParameter` 设为 `max_tokens`。生图模型 `baidu/ERNIE-Image-Turbo` 可配合 `researchBlog.images.size: "1536x1024"` 使用。模型以账号实际可用列表为准。

运行 **Research Blog: 设置 AI API Key** 保存密钥；当两处接口地址相同时，生图会复用文字补全密钥。不要把密钥写进 JSON 设置或文章。插件自动适配硅基流动的 `image_size` 和 `images[].url`，下载后保存在文章目录。ERNIE-Image-Turbo 使用 8 步；硅基流动文字行内补全关闭深度思考以控制延迟，风格由统一提示词约束。

已配置文字模型时，生成配图默认先把选区和统一风格交给文字模型整理为画面构思，再发送给生图接口，因此会调用两次 API。`researchBlog.images.preparePrompt` 可关闭此步骤；未配置文字模型时可直接生图。文字补全与构思使用同一文字模型。

在设置界面搜索 `@ext:mryu888-plus.research-blog`，或编辑相应的 JSON 设置。机器级选项请放在**用户设置**中；其余选项可按工作区覆盖。

| 配置键 | 默认值 | 用法与范围 |
| --- | --- | --- |
| `researchBlog.contentPath` | `"site/content"` | 相对工作区的文章目录；必须在工作区内，修改后重启语言服务；可按工作区文件夹设置 |
| `researchBlog.trace.server` | `"off"` | LSP 通信日志：`"off"`、`"messages"`、`"verbose"` |
| `researchBlog.ai.enabled` | `false` | 开启公式、围栏代码块和 Typst 的 AI 行内补全 |
| `researchBlog.ai.baseUrl` | `"https://api.openai.com/v1"` | Chat Completions 兼容接口基础地址；机器级，仅在用户设置配置 |
| `researchBlog.ai.model` | `""` | 模型 ID；必须显式填写；机器级 |
| `researchBlog.ai.debounceMs` | `600` | 自动补全前等待毫秒数，范围 `200–3000`；手动触发跳过此等待 |
| `researchBlog.ai.contextChars` | `4000` | 光标前后上下文字符总预算，范围 `500–12000`；后文最多占四分之一 |
| `researchBlog.ai.maxTokens` | `256` | 请求输出 token 上限，范围 `32–2048` |
| `researchBlog.ai.tokenParameter` | `"max_completion_tokens"` | 接口只支持旧参数时选择 `"max_tokens"`；机器级 |

## 常见问题

| 现象 | 检查与处理 |
| --- | --- |
| 双链、tags 没有补全 | 确认已打开博客文件夹，Markdown 位于 `contentPath` 内，语言模式为 Markdown；按 `Ctrl+Space`。修改目录设置后运行 **Research Blog: 重启语言服务** |
| 某篇文章不在双链候选中 | 目标须在 `posts/` 下且不是 `_index.md`；检查 TOML 是否有效、`title` 是否非空。同名文章使用候选中的内容路径 |
| 标签没有中文名称或列表为空 | 使用 `[taxonomies]` 下的 `tags = [...]`，在标签引号内部或逗号后触发；检查文章目录上一层 `config.toml` 的 `[extra.tag_labels]` 并保存。没有已用标签或配置标签时，直接输入新标签 |
| 中文章节链接警告 | 使用章节原文，如 `[[文章#问题与背景]]`，或给标题添加显式 `{#background}` 再引用 `#background` |
| 图形补全或 F12 没反应 | 确认 `.typ` 位于当前文章目录的 `figures/` 中，ID 不含扩展名和路径分隔符；补全使用同一行的 `id="…"` 写法 |
| Typst 没有诊断或跳转 | 确认 Tinymist 已安装且启用，右下角语言模式为 Typst；用 **Research Blog: 打开 Tinymist Typst 语言支持** 查看扩展详情 |
| AI 没有灰色建议 | 确认工作区受信任，文件在 `contentPath` 内，启用了 `researchBlog.ai.enabled` 和 `editor.inlineSuggest.enabled`，模型不为空；光标必须在支持的位置。用手动命令检查提示，并查看 **Output → Research Blog AI** |
| 更换 AI 接口后不工作 | 为新地址重新运行 **Research Blog: 设置 AI API Key**；检查地址未重复包含 `/chat/completions`，远程地址使用 HTTPS |
| AI 返回 HTTP 400、401 或 404 | 检查服务商实际支持的模型、接口地址、Key 和 token 参数；仅支持旧参数的接口设为 `max_tokens`。错误不会包含接口响应正文，详细原因需结合服务商侧记录确认 |
| AI 返回 HTTP 429 或始终超时 | 检查服务商额度、限流和网络；可增加 `debounceMs` 或降低 `maxTokens`。生成超过 15 秒会取消；仅支持流式输出的接口不适用 |
| 网站没有显示草稿或图形 | 编辑辅助不等于发布。用 `python scripts/editor.py preview` 查看草稿；正式发布前检查 `draft`，并完成网站构建 |

语言服务的输出通道为 **Research Blog (工作区名称)**。需要进一步诊断时，临时将 `researchBlog.trace.server` 设为 `"verbose"`，复现后改回 `"off"`。详细 LSP 通信日志可能包含文档内容，分享前请检查。

## 源码开发和验证

以下步骤仅供修改插件源码时使用。在完整博客仓库中执行，需要 Node.js 和 npm：

```powershell
cd extensions/research-blog
npm ci --ignore-scripts
npm run build
npm test
npm run package
```

源码通过 esbuild 捆绑，VSIX 包含运行时依赖和第三方许可，不含密钥、测试目录、文章或构建素材。构建直接捆绑仓库双链模块，避免复制逻辑后发生分歧。测试覆盖解析与补全引擎、真实 stdio LSP 通信、本机 HTTP AI 请求；模拟接口测试不能证明真实模型的补全质量。

源码入口：[`src/extension.cjs`](src/extension.cjs)、[`src/engine.cjs`](src/engine.cjs)、[`src/ai.cjs`](src/ai.cjs)；配置定义见 [`package.json`](package.json)。
