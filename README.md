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

## 写文章

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

正文使用 Markdown；公式支持 `$x^2$` 和 `$$ ... $$`。在文章目录的 `figures/` 下放置 `.typ` 文件，并使用 Zola 组件引用：

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
node --test tests/frontend.test.cjs
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
