# 从干净模板开始

需要 Python 3.11+ 和 Node.js 18+。Zola 0.23.4 和 Typst 0.15.1 可以由 `python scripts/setup_tools.py` 装到仓库的 `.tools/` 目录，也可以自己装好放进 `PATH`。Python 脚本只用标准库。写作插件需要 VS Code；Rust 只在使用 Rust CLI 和文章助手时需要。

这些步骤也可以交给 agent：在仓库目录打开 Claude Code 或 Codex，让它按 `AGENTS.md` 建站点、装工具、打开预览。下面是自己动手的步骤。

在下载的源码目录里执行：

```sh
python scripts/init_site.py ../my-research-notes --name "我的研究手记" --author "你的名字" --url "https://USERNAME.github.io/REPO/"
cd ../my-research-notes
python scripts/setup_tools.py
npm --prefix extensions/research-blog ci --ignore-scripts
npm --prefix extensions/research-blog run build
python scripts/build_site.py
python scripts/check_site.py
python scripts/editor.py preview
```

目标目录必须不存在或为空。加 `--dry-run` 只检查路径、显示摘要，不写文件。初始化会复制通用代码、`ZEN.md`、`AGENTS.md` 和匿名示例，不会复制作者的文章、简历、联系方式、私人文献、密钥、生成结果或 Git 历史。新站点默认 `extra.profile_enabled = false`，关于页就是一个普通的 Markdown 页面。

预览命令会打印一个本地地址，预览里包含草稿。保存文章后页面会更新，Typst 图形和 `ZEN.md` 的修改也会被监听。按 `Ctrl+C` 结束预览。公开构建输出在 `site/public/`，不包含 `draft = true` 的文章。

## 编辑器和文献

从源码构建两款插件：

```sh
npm --prefix extensions/research-blog ci --ignore-scripts
npm --prefix extensions/research-blog run package
npm --prefix extensions/research-notes-theme ci --ignore-scripts
npm --prefix extensions/research-notes-theme run package
```

在 VS Code 里选择“从 VSIX 安装”，分别安装两个目录里生成的文件。然后打开 `site/content/posts/welcome/index.md`，运行 **Research Blog: 预览网站（含草稿）**。

运行 **Research Blog: 打开文献工作台** 连接 Zotero。在工作台里修改文献需要 Zotero 10+；读 PDF 和编辑批注仍在 Zotero 自带的阅读器里完成。第一次修改时要在 Zotero 里点允许；要在多台设备之间同步，请登录 Zotero 并设置附件同步。插件支持分类、导入 PDF、用批注生成阅读草稿和 `[@citekey]` 引用。示例里的引用数据是公开的 Zotero 入门指南，不是私人文献库。

## 风格和文章格式

新站点默认用 ZEN 风格。要改成自己的，编辑 `ZEN.md` 里 `zen:tokens` 和 `zen:prompts` 两段 JSON：深色和浅色配色、字体、正文宽度和行高、间距、图形规则都在里面。格式和两套调色板的键名不要改。参数说明和修改示例见[自定义风格指南](style-guide.md)。

改完运行 `node scripts/sync_zen.cjs`，构建网站和实时预览时也会自动同步。生成的 `zen.css`、主题 JSON 和模板参数区不要手改。已安装的编辑器插件要重新打包、安装才会更新；VS Code 的编辑器字体仍由你的设置决定。组件的细节布局可以直接改 CSS。

示例文章里有 `$...$` 公式、`figures/*.typ` 图形、`[[文章标题]]` 双链和 `[@citekey]` 引用。文章的元数据写在文章开头的 TOML 里，网站设置在 `site/config.toml`。文章助手和 AI 配图需要自己配置服务和密钥；写作、预览和发布都不需要 AI。

## 发布自己的站点

在 GitHub 上新建一个空仓库，在 **Settings → Pages → Source** 里选 **GitHub Actions**。确认 `site/config.toml` 里的 `base_url` 和实际地址一致，比如 `https://USERNAME.github.io/REPO/`，然后在新站点目录里执行：

```sh
git init -b main
git add .
git commit -m "Start research notes"
git remote add origin https://github.com/USERNAME/REPO.git
git push -u origin main
```

模板自带 `.github/workflows/pages.yml`：安装固定版本的构建工具，检查公开链接和不该公开的内容，然后发布 `site/public/`。PR 只构建和检查，不部署。执行结果和网站地址在仓库的 Actions 页面里查看。

`draft = true` 只影响网站构建。草稿文件提交到公开的 Git 仓库后同样是公开的，私人笔记请放在仓库外。发布流程不会替你创建 GitHub 仓库，也不会替你推送代码。
