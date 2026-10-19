# 我的研究笔记

这个目录是用 Research Blog 生成的个人博客：实时预览、Markdown 公式、Typst 图形、双链、Zotero 引用，风格写在一个文件 `ZEN.md` 里。

A personal blog created with Research Blog: live preview, Markdown math, Typst figures, linked notes and Zotero citations, with the style kept in one file, `ZEN.md`. Everything runs through one program, `blogctl`.

可以把这个目录交给 Claude Code 或 Codex 这样的 agent：`AGENTS.md` 写了文件在哪、每种修改之后运行什么、哪些事要先问你。下面是自己动手的步骤。

## 准备

1. 从 [Research Blog 的发布页](https://github.com/mryu888-plus/research-blog/releases) 下载对应系统的 `blogctl`，解压后放进 PATH。
2. 装一种中文字体。Windows 和 macOS 自带；Linux 可以装 `fonts-noto-cjk`。

Zola 和 Typst 不用自己装：第一次构建或预览时，`blogctl` 会下载固定版本并校验，之后所有站点共用。

## 写作

```sh
blogctl preview          # 实时预览，含草稿，地址打印在终端
blogctl new "文章标题"    # 新建草稿
blogctl check            # 构建并检查整个网站
```

文章在 `site/content/posts/` 下，每篇一个目录。没写完的文章保持 `draft = true`，它不会出现在网站上；但提交到公开 Git 仓库的文件别人照样能看到，私人笔记请放在仓库外面。公开网站只收录已发布文章引用的书目信息，本地 PDF、密钥和私人缓存不会进入网站。

站名、作者和网址在 `site/config.toml`。关于页是普通的 Markdown：`site/content/about.md`。

## 风格

默认风格叫 ZEN。颜色、字体、正文宽度和行高、间距、图形参数和配图规则都在 `ZEN.md` 的两段 JSON 里。改完运行 `blogctl zen`；开着预览时保存 `ZEN.md` 就会生效。不要手改生成的 `site/static/zen.css` 和 Typst 模板参数段；组件的布局可以直接改 CSS。

## VS Code

写作插件和编辑器主题也在发布页，用 VS Code 的「从 VSIX 安装」装上。打开一篇文章，运行 **Research Blog: 预览网站（含草稿）**。**Research Blog: 打开文献工作台** 连接本机的 Zotero 10+；同步请自己登录 Zotero。

## 发布到 GitHub Pages

在 GitHub 新建一个空仓库，在 **Settings → Pages → Source** 里选 **GitHub Actions**。把 `site/config.toml` 的 `base_url` 改成实际地址，比如 `https://USERNAME.github.io/REPO/`，然后：

```sh
git init -b main
git add .
git commit -m "Start research notes"
git remote add origin https://github.com/USERNAME/REPO.git
git push -u origin main
```

`.github/workflows/pages.yml` 会下载生成这个站点时的同一版本 `blogctl`，构建、检查后再发布。PR 只构建和检查，不部署。结果和网址在仓库的 Actions 页面查看。

模板代码和示例使用 [MIT 许可](LICENSE)，另见[内容许可](CONTENT_LICENSE.md)和[第三方通知](THIRD_PARTY_NOTICES.md)。你自己的文章和素材用什么许可由你决定。
