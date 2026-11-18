# Research Notes · 研究手记

默认是 Research Blog 的 ZEN 配色：石墨黑、暖纸白、灰色文字和陶土红。颜色可以在 ZEN.md 里改成自己的。

主题和博客、Typst、AI 配图共用 [ZEN.md](../../ZEN.md)。基础色、语义色和状态色都在其中的调色板里维护，构建时生成两套主题和本目录的预览页。参数和修改示例见[自定义风格指南](../../docs/style-guide.md)。

| 默认主题 | 编辑器底色 | 正文 | 点缀 |
| --- | --- | --- | --- |
| **Research Notes Graphite** | `#121213` | `#ededee` | `#de8b82` |
| **Research Notes Paper** | `#f8f7f4` | `#242427` | `#9d4941` |

默认界面以灰阶为主，活动标签、光标与关键词使用陶土红。字符串、类型与数字采用低饱和辅助色；错误、警告、Git 差异和终端 ANSI 颜色保留语义区分。这些颜色都可以在风格源中修改。支持 TextMate 和 semantic tokens，并单独照顾 Markdown 标题、链接、引用与行内代码。实际语法识别依赖 VS Code 或相应语言插件。

## 安装

可以在 VS Code 扩展面板搜索 `mryu888-plus.research-notes-theme`（[Marketplace 页面](https://marketplace.visualstudio.com/items?itemName=mryu888-plus.research-notes-theme)）直接安装；如果自己在 `ZEN.md` 里改了配色并重新打包，按下面步骤本地安装：

1. 在 VS Code 扩展面板右上角 `…` 中选择 **Install from VSIX… / 从 VSIX 安装…**。
2. 选择 `research-notes-theme-0.1.0.vsix`。
3. 按 `Ctrl+K`，再按 `Ctrl+T`，搜索 **Research Notes**，选择 Graphite 或 Paper。

也可以在安装包所在目录执行：

```powershell
code --install-extension ./research-notes-theme-0.1.0.vsix
```

这是独立的纯配色扩展，无运行时代码和依赖，不会修改编辑器设置、字体或安装语言插件。无需安装 Research Blog Language Tools。

## 预览与可选设置

在源码目录打开 `preview.html` 可切换两套配色，以及 Python、Rust、Markdown 示例。预览颜色由同一份主题数据生成；浏览器绘制的编辑器外观和手工分类的代码示例仅用于预览，不能代替真实 VS Code 及语言插件的渲染结果。

如果想让编辑器的字体和行距接近博客，可以把下面的设置合并到 VS Code 用户设置里；扩展不会自动写入这些设置：

```json
{
  "workbench.colorTheme": "Research Notes Graphite",
  "editor.fontFamily": "'Cascadia Code', Consolas, monospace",
  "editor.fontSize": 14,
  "editor.lineHeight": 24,
  "editor.cursorStyle": "line-thin",
  "editor.minimap.enabled": false,
  "editor.renderLineHighlight": "line"
}
```

字体需要自己安装。配色主题只能控制颜色和部分文字样式，博客里的留白、扫描线和光晕在 VS Code 主题里做不到。

## 开发

```sh
npm ci --ignore-scripts
npm run build
npm run package
```

修改仓库根目录 `ZEN.md` 中的 `palettes.graphite` 和 `palettes.paper`，其中 `editor` 保存语法色、诊断色、选择区域和终端颜色，再重新构建、打包并安装 VSIX。可以修改风格 `name`，但两套主题仍保留 **Research Notes Graphite**、**Research Notes Paper** 名称和扩展标识；现有主题设置可以继续使用。`fonts` 不会修改 VS Code 编辑器字体，需要用户自行设置。

`themes/` 下的 JSON 和 `preview.html` 是生成文件，请勿手改。它们由仓库根目录的 `blogctl zen`（源码仓库里是 `cargo run -p blogctl -- zen`）从 `ZEN.md` 生成；VS Code 的颜色角色映射在 `crates/blog-site/src/zen/theme.json`。`npm run build` 只检查生成文件是否最新，不是最新时构建失败。网站实时预览会监听风格源，已安装的主题则使用打包数据。源码构建需要完整博客仓库；安装后的纯主题 VSIX 不依赖仓库。

配色格式遵循 [VS Code Color Theme](https://code.visualstudio.com/api/extension-guides/color-theme)、[Theme Color Reference](https://code.visualstudio.com/api/references/theme-color) 与 [Semantic Highlight Guide](https://code.visualstudio.com/api/language-extensions/semantic-highlight-guide)。
