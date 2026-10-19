# 工具与内容的许可范围

Research Blog 的工具代码和可复用模板采用根目录 [MIT License](LICENSE)。仓库同时保留作者实际使用的个人站点，个人内容不因工具开源而获得 MIT 授权。

## MIT 范围

除文件另有第三方通知或本页列明的内容外，以下项目由 Research Blog contributors 按 MIT 提供：

- Rust、Python 和 JavaScript 工具代码、测试、工作流与编辑器扩展。
- 网站模板、组件 CSS、交互代码和站点配置结构。
- `ZEN.md` 的设计参数与规则，以及由其生成的网站变量、VS Code 主题、Typst 模板参数和代码片段。
- 可复用 Typst 排版代码，如 `profile/layout.typ` 与 `design/visual-system/typst/blog-diagram.typ`；具体简历和研究图形中的个人内容另列。
- 项目说明文档、演示页面代码、界面截图及 `scripts/init_site.py` 生成的入门示例内容。

可以复制这些工具、模板和示例创建自己的站点，按 MIT 要求保留版权与许可通知。使用者自己的文章与素材由使用者决定许可。

## 保留作者权利的内容

以下内容保留各自作者的权利；除文件明确注明其他许可或另获授权，未授予转载、改编或再分发许可：

- 本仓库 `site/content/` 中的个人文章、阅读笔记、摘录和文章素材。生成器新建的通用入门示例不在此项之内。
- `profile/cv.typ` 中的姓名、经历、论文、项目与其他简历内容，以及相应个人页和简历 PDF。
- 个人文章中的研究图形、原始图片，以及 `design/visual-system/assets/`、`design/visual-system/examples/` 和 `extensions/research-blog/assets/editorial-reference.png` 中保留的配图探索素材及其来源记录。
- 作者或使用者自己的文献 PDF、批注、书目库和其他研究资料。

界面截图可用于介绍本项目；这不授予其中展示的论文、文章或第三方素材的独立再利用权。个人信息也不能作为新站点的默认作者资料。`scripts/init_site.py` 为此只复制工具与通用示例，不复制个人文章、CV、研究素材或私有文献库。

## 第三方内容

依赖、字体、引用的论文、上游软件和第三方素材保持原有许可或权利状态。根目录 MIT 不能改变这些条款；AI 配图的来源记录也不能替代生成服务的适用条款。具体依赖与来源入口见 [THIRD_PARTY_NOTICES](THIRD_PARTY_NOTICES.md)。

引用信息和摘录是研究写作输入，不代表可公开分发整篇论文。发布阅读笔记前，请检查自己的文字、引文和图片来源；工具不会为内容自动授予授权。
