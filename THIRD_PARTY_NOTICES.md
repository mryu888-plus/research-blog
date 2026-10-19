# 第三方依赖与来源

本文件记录当前源码和构建中可确认的第三方来源。第三方组件按自身许可提供，不受项目根目录 MIT 重新授权。版本应以 `Cargo.lock`、扩展 `package-lock.json`、实际模板和 Typst 导入为准。

## VS Code 安装包

插件构建使用 esbuild 的依赖清单收集实际进入 bundle 的组件许可证，保存为 `extensions/research-blog/dist/THIRD_PARTY_LICENSES.txt`，并保留 esbuild 生成的法律注释。VSIX 带有这些文件，使用和再分发时应保留。

直接依赖包括 Microsoft 的 `vscode-languageclient`、`vscode-languageserver`、`vscode-languageserver-textdocument` 和 `vscode-uri`（MIT），`markdown-it`（MIT），以及 `smol-toml`（BSD-3-Clause）。间接依赖在生成的许可证文件中逐项列出，例如 `semver` 使用 ISC。不要将所有依赖笼统视为 MIT；分发安装包以随包的原始许可为准。

构建工具和未进入 bundle 的开发依赖保持 npm 包自己的通知。Rust 依赖由 Cargo 按锁文件获取，其源码与许可证位于各自上游 crate；发布编译后的 Rust 程序时需要按所包含的依赖保留适用通知。

## 网站与图形

| 来源 | 当前用途 | 许可与保留方式 |
| --- | --- | --- |
| [KaTeX 0.16.9](https://github.com/KaTeX/KaTeX/blob/v0.16.9/LICENSE) | `site/templates/base.html` 从 jsDelivr 加载公式排版脚本与 CSS。 | MIT；资源保留上游通知。若改为本地分发，应随资源带原始 LICENSE。 |
| [CeTZ](https://github.com/cetz-package/cetz) | Typst 图形可导入 `@preview/cetz:0.4.2`，编译器下载到用户包缓存。 | 0.4.2 的 `typst.toml` 声明 LGPL-3.0-or-later，并带原始 LICENSE。项目未把其包源码纳入仓库；复制或修改 CeTZ 源码时保持其许可。 |
| [Apollo](https://github.com/not-matthias/apollo) | 极简网站排版的视觉参考；`site/static/apollo.css` 明确标注 Apollo-inspired。 | 上游为 MIT。本仓库使用自己的既有 Zola 模板和阅读交互，没有引入 Apollo 主题子模块；后续引入上游代码应保留原版权与许可证。 |

网站字体优先引用系统已安装的字体。CI 安装 Noto CJK，未将字体二进制纳入本仓库。若在自己的站点托管字体文件，请同时保留该字体的许可。

`design/visual-system/README.md` 记录探索图片的生成来源，相关 `.prompt.txt` 与 JSON 保留提示词及生成记录。研究文章中的引用、论文图片和外部内容由各自作者持有权利，范围见 [CONTENT_LICENSE](CONTENT_LICENSE.md)。

## 外部工具与服务

[Zola](https://github.com/getzola/zola)、[Typst](https://github.com/typst/typst)、[Zotero](https://github.com/zotero/zotero) 与可选 AI 服务是分别安装或配置的外部工具。其程序、服务条款与商标由各自项目或提供方管理；本项目不表示获得其背书。

Windows Zotero 助手从官方地址下载客户端到 Git 忽略的 `.tools/zotero/`，不将二进制、用户库或配置打进源码、站点与 VSIX。重新分发这些外部程序应查阅对应版本的原始许可；本站点生成器不会复制它们。
