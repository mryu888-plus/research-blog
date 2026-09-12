# Research Blog Language Tools

面向本仓库 Zola 主题的 VS Code 写作插件。Markdown 保持原语言模式，博客语法由独立 stdio LSP 处理，Typst 交给 Tinymist，公式和代码可启用 AI 行内补全。

## 安装

在 VS Code 的扩展面板菜单选择 **Install from VSIX…**，选择 `research-blog-0.1.0.vsix`。也可运行：

```powershell
code --install-extension extensions/research-blog/research-blog-0.1.0.vsix
```

安装包声明 Tinymist (`myriad-dreamin.tinymist`) 为配套扩展，由 VS Code 安装。离线安装时可单独安装 Tinymist 的 VSIX；博客语言服务本身不需要额外的 Node、Python、Zola 或 Rust 进程。插件尚未发布到 Marketplace。

打开博客仓库根目录；默认索引 `site/content/**/*.md`。如果直接打开 `site`，将 `researchBlog.contentPath` 改为 `content`，再运行 **Research Blog: 重启语言服务**。多根工作区各自使用独立服务器。

## 写作功能

| 输入或操作 | 效果 |
| --- | --- |
| `[[` 后输入标题／别名，或按 Ctrl+Space | 补全文章；同名文章用内容路径区分 |
| `[[文章#` 或 `[[#` | 补全目标文章／本文的章节标题 |
| 在双链上按 F12、悬停、Shift+F12 | 跳转、摘要、查找引用 |
| 在文章 title 或章节标题上按 Shift+F12 | 查找所有引用文章／该章节的双链 |
| frontmatter 字段处按 Ctrl+Space | title、date、draft、description、tags、author、aliases 等字段补全 |
| `[taxonomies]` 的 `tags = ["…"]` | 补全已有标签和 `config.toml` 的 `extra.tag_labels` 键，展示中文名和文章数；支持多行数组 |
| 标签处悬停／Shift+F12 | 查看使用次数、文章列表、定位使用位置 |
| `{{ <fig id="…" /> }}` | 补全当前 bundle 的 `figures/*.typ`；F12 打开源文件 |
| Problems 面板 | TOML 语法、主要元数据类型、重复标签、失效／重名／缺章节双链、公开文章引用草稿、缺失图形 |
| `blogpost`、`bloglink`、`blogsectionlink`、`blogfig` | 插入主题模板 |

补全会保留双链显示文字和已有闭合符号。链接解析复用网站 `knowledge-core.js`，跳转可读取未保存的内容，创建、修改、删除文章后索引自动更新。代码块、行内代码、注释和公式中的双括号不报链接错误。

章节建议优先使用可读标题；支持显式 `{#id}` 和常规英文锚点。Zola 自动音译的中文 URL 锚点暂不解析，请写 `[[文章#中文章节标题]]`。双链目标与主题图谱保持一致，来自 `posts/` 页面，不包括 section 索引。普通 Markdown 链接的跳转继续由 VS Code 内置服务负责。本插件不提供重命名或自动改写链接。

## Typst

打开文章 `figures/*.typ`，Tinymist 提供 Typst 补全、诊断、跳转、悬停、格式化和预览。命令 **Research Blog: 打开 Tinymist Typst 语言支持** 可打开其扩展详情。Markdown 中图形 ID 的 F12 可直接进入 `.typ` 文件。

Tinymist 的语义分析作用于 `.typ` 文件；Markdown 内嵌公式是 LaTeX，由现有预览渲染，并可使用下面的 AI 补全。两种公式语法各自处理。网站预览仍使用仓库已有的 `python scripts/editor.py preview`。

## 可选 AI 补全

AI 默认关闭。打开 VS Code 的**用户设置 JSON**，填写自己接口支持的模型 ID：

```json
{
  "researchBlog.ai.enabled": true,
  "researchBlog.ai.baseUrl": "https://api.openai.com/v1",
  "researchBlog.ai.model": "你的模型ID",
  "researchBlog.ai.tokenParameter": "max_completion_tokens"
}
```

运行 **Research Blog: 设置 AI API Key**。Key 按接口地址保存在 VS Code SecretStorage，可用 **Research Blog: 删除 AI API Key** 删除。不要把 Key 写入仓库或 settings.json。修改接口地址后需要为新接口重新设置 Key。

支持 OpenAI Chat Completions 兼容接口，基础地址不含 `/chat/completions`。若提供商只支持 `max_tokens`，修改 `researchBlog.ai.tokenParameter`。本地兼容服务可配置为 `http://127.0.0.1:端口/v1`，本机服务可不设置 Key；远程接口需要 HTTPS 和 Key。模型和地址只接受用户／机器级设置。

在 Markdown 的 `$…$`、`$$…$$`、`\(…\)`、`\[…\]`、围栏代码块，或文章目录内的 `.typ` 文件中输入，停顿 600 ms 后显示灰色建议，**Tab 接受、Esc 忽略**；也可运行 **Research Blog: AI 补全公式或代码**。普通正文不会自动请求 AI。

默认每次仅发送当前文件光标附近最多 4,000 个字符（前文和后文合计），最多请求 256 个输出 token。不发送其他文章、文件路径或整个仓库；日志不记录正文、Key 或接口响应正文。接受建议前不会写入文档或执行代码。继续输入会取消旧请求，15 秒后超时，受限工作区不会请求 AI。接口按服务商规则计费；可随时关闭 enabled。

调试时查看 Output → **Research Blog AI**；LSP 日志可设置 `researchBlog.trace.server = "verbose"`。AI 部分已通过本机 HTTP 模拟接口验证，真实模型质量和延迟取决于所配置服务，需要你提供接口后验证。

## 开发和验证

```powershell
cd extensions/research-blog
npm ci
npm run build
npm test
npm run package
```

源码通过 esbuild 捆绑，VSIX 包含运行时依赖和第三方许可，不含密钥、测试目录、文章或构建素材。构建时直接捆绑仓库双链模块，避免复制逻辑后发生分歧。测试覆盖引擎、真实 stdio LSP 通信与本机 HTTP AI 请求。

协议与接口依据：[VS Code LSP 指南](https://code.visualstudio.com/api/language-extensions/language-server-extension-guide)、[Tinymist 官方文档](https://myriad-dreamin.github.io/tinymist/frontend/vscode.html)、[OpenAI Chat API](https://developers.openai.com/api/reference/resources/chat)。
