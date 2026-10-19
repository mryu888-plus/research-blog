# 定义自己的风格

[返回项目首页](../README.md) · [默认风格源](../ZEN.md) · [从干净模板开始](getting-started.md)

ZEN 是项目自带的默认风格：石墨黑、暖纸白，加少量陶土红。可以直接用，也可以换成自己的配色、字体、版面尺寸和图形规则。所有风格参数都写在根目录的 `ZEN.md` 里，网站样式、编辑器配色、Typst 模板和 AI 规则都从这里生成。

## 编辑哪一部分

`ZEN.md` 包含两个带标记的 JSON 块：

- `zen:tokens`：风格名称、颜色、字体、布局尺度、基础间距、圆角、反馈时长与图形参数。
- `zen:prompts`：AI 配图和 Typst 补全使用的英文规则。

保留文件名、这两个块的标记和 `schemaVersion: 1`。`name` 可以改为任意非空名称；它描述你的风格，不会更改网站标题、插件标识或 VS Code 主题名称。网站名称和作者在 `site/config.toml` 中修改。JSON 以外的文字用于说明设计想法，同步器只读取这两个 JSON 块。

## 可以修改什么

| 参数 | 用途与边界 |
| --- | --- |
| `name`、`version` | 记录自己的风格名称与版本；`schemaVersion` 保留为 `1`。 |
| `palettes.graphite`、`palettes.paper` | 分别对应深色和浅色。可修改背景、各级表面、文字、边线、强调色等；保留两套调色板的键名及现有字段。 |
| 调色板的 `editor` | VS Code 专用表面、语法色、诊断色、选择区域和 ANSI 色。它们独立维护，修改网站强调色不会自动重配全部语法色。 |
| 调色板的 `contrast` | 浏览器启用高对比度偏好时的次要文字、辅助文字与边线颜色。 |
| 调色板的 `effects` | 网站背景光晕、扫描线、导航背景、边缘亮色与阴影。`halo`、`scanline` 可设为 `transparent`；阴影可设为 `none`。 |
| `print` | 网站打印时使用的背景、文字、边线与强调色。 |
| `fonts.sans`、`fonts.mono`、`fonts.serif` | 网站可用的字体栈；从左到右回退。数学公式保留数学排版所需字体。 |
| `fonts.figure` | 生成的 Typst 图形模板使用的字体栈。 |
| `layout.pageWidth` | 网站页面容器的最大宽度，单位为 px。 |
| `layout.articleWidth`、`layout.articleLineHeight` | 博客正文列宽度与行高；可省略，默认分别为 `700` px 和 `1.95`。 |
| `layout.readingWidth`、`layout.bodyLineHeight` | 独立视觉示例的阅读宽度，以及网站基础行高；正文文章有自己的行高参数。 |
| `spacing` | 七档基础间距，单位为 px。保持七个正数，分别生成 `--spacing-xs`、`sm`、`md`、`lg`、`xl`、`2xl`、`3xl`。 |
| `radius`、`motion.feedbackMs` | 基础圆角（px）与普通交互反馈时长（ms），可设为 `0`。组件自有的形状和图谱动画仍由各自 CSS 控制。 |
| `figure` | Typst 模板的细线与主线宽度、标签字号、页面边距，单位为 pt。 |
| 调色板的 `imageWords` | AI 配图使用的色调描述。换色时同时修改它，避免提示词仍要求旧配色。 |
| `prompts.image`、`prompts.typst` | 配图的开场、处理方式、用途构图和约束，以及 Typst 补全规则。 |

基础调色板和打印颜色使用 `#RRGGBB`；`editor` 颜色也支持 `#RRGGBBAA`。`effects` 使用相应 CSS 属性接受的值。修改背景时，可一并调整导航背景、阴影和高对比度颜色，使它们符合新的配色。

字体不会自动下载。网站访客没有某个字体时使用后面的回退字体；本机 Typst 使用已安装的字体。VS Code 配色主题不设置编辑器字体、字号和行距，这些需要在用户设置中自行选择。

## 一个局部修改示例

下面示例把浅色强调色改为蓝色，让正文稍宽，并减少普通反馈动画。**将这些字段合并到原 `zen:tokens` JSON 的对应位置；不要用这段局部示例替换整个 JSON 块。** 省略的颜色、字体、深色调色板和其他字段都要保留。

```json
{
  "name": "My Reading Style",
  "layout": {
    "articleWidth": 760,
    "articleLineHeight": 1.9
  },
  "motion": {
    "feedbackMs": 0
  },
  "palettes": {
    "paper": {
      "accent": "#315b93",
      "imageWords": "flat light background, clear dark ink lines, restrained gray, one muted blue accent"
    }
  }
}
```

例如，原 `layout` 中的 `pageWidth`、`readingWidth`、`bodyLineHeight` 仍需保留，再加入 `articleWidth` 和 `articleLineHeight`。同样保留 `palettes.graphite`，只修改 `palettes.paper` 中示例列出的字段。若也想改变编辑器按钮或语法色，继续调整该调色板的 `editor`。

英文规则也可以改。例如，把下面字段合并到原 `zen:prompts` JSON 的对应位置，保留其他处理方式、构图用途和科学约束：

```json
{
  "image": {
    "opening": "Create a clear explanatory illustration for a research note using the selected palette. Keep the subject easy to recognize."
  }
}
```

换了风格名称后，可以同步修改开场规则中对旧风格的描述。调整配图和 Typst 规则时，建议保留关系、数值、方程和箭头方向必须忠于来源的要求。

## 同步与检查

在项目根目录运行：

```sh
node scripts/sync_zen.cjs
node scripts/sync_zen.cjs --check
```

第一条命令验证参数并生成接入文件；第二条只检查是否存在遗漏同步的文件。网站和插件的常规构建也会执行同步。运行 `python scripts/editor.py preview` 后，保存 `ZEN.md` 会更新网站预览，方便检查明暗主题和不同屏幕宽度。

已安装的 VSIX 使用打包时的风格数据。要更新编辑器配色，重新打包、安装主题插件；修改图形片段或 AI 规则后，重新打包、安装写作插件：

```sh
npm --prefix extensions/research-notes-theme run package
npm --prefix extensions/research-blog run package
```

然后在 VS Code 中从 VSIX 安装对应的新包。源码依赖尚未安装时，先在各目录执行 `npm ci --ignore-scripts`。两款主题仍使用固定的 **Research Notes Graphite**、**Research Notes Paper** 名称和标识，因此可继续使用现有设置。更新公开网站则需要正常构建与发布。

## 进一步调整布局

风格源控制共用基础参数。组件的排列方式、各级标题字号、部分留白与圆角仍在 `site/static/apollo.css` 和其他组件 CSS 中定义；可以按自己的阅读习惯修改，并继续引用语义变量。主题扩展主要控制颜色与部分代码文字样式，网站的间距、扫描线和光晕不会同步到 VS Code 界面布局。

不要手改生成的 `site/static/zen.css`、`design/zen.generated.cjs`、主题 JSON、主题 `preview.html`、Typst 模板的生成参数区或图形片段。这些改动会在下次同步时被覆盖。同步器不会重画已有图片，也不会改写已经复制进文章的 Typst 图形；文章中的图形由作者维护。
