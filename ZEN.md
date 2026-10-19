# ZEN · 研究手记设计语言

ZEN 是这个项目的默认风格：**石墨黑和暖纸白，细线分隔，留白充足，只用一点陶土红。** 可以直接用，也可以在这里改成自己的颜色、字体、版面尺寸、图形参数和配图规则。

设计规则和参数只在这个文件里维护。`blogctl zen` 读取下面 `zen:tokens` 和 `zen:prompts` 两段 JSON，生成网站 CSS、VS Code 主题、Typst 模板和 AI 提示词。改完运行 `blogctl zen`；构建网站和打开预览时也会自动同步。

每类参数的用途和改法见[自定义指南](docs/style-guide.md)。文件名、两段 JSON 的标记和 `schemaVersion` 不要动；`name` 可以换成你自己的风格名，`graphite` 和 `paper` 分别是深色和浅色配置，键名保持不变。下面的文字说明的是默认方案，改了参数可以一起改；`blogctl zen` 只读 JSON。

## 基本原则

- 大部分内容只用黑、白和灰。Graphite 是深色的石墨黑，Paper 是浅色的暖纸白。陶土红只用来标出当前选中的东西、最关键的一条关系，或者刚刚发生的变化。
- 文字平实，可以有一点文学感。「研究手记」、首页题词 Γ ⊨ φ ⟺ Γ ⊢ φ 和它下面的 "Wir müssen wissen. Wir werden wissen." 这些已有的说法保留。页面名称、按钮和提示要一眼能看懂。
- 层次先靠对齐、间距和细线。长文、目录和列表不加框；弹窗、代码块和操作区可以有浅边框和小圆角。
- 每个区域只有一个重点。图注、日期和编号放在次要的位置；装饰不能盖过正文，也不能妨碍操作。

## 色彩

| 角色 | Graphite 石墨黑 | Paper 暖纸白 |
| --- | --- | --- |
| 背景 | `#121213` | `#f8f7f4` |
| 主要文字 / 轮廓 | `#ededee` | `#242427` |
| 次要文字 | `#aaaab2` | `#5e5e68` |
| 辅助文字 | `#92929b` | `#666670` |
| 分隔线 | `#2c2c30` | `#deddd8` |
| 强调 | `#de8b82` | `#9d4941` |

红色只做点缀，在图形里一般不超过 3% 的面积，最好只强调一个关系。代码高亮、错误、警告、Git 差异和多类别图表可以用低饱和度的辅助色，但同时要用文字、形状或线型来区分。类别和数据要准确，颜色多少是次要的。

深色和浅色主题用同一套语义角色，位图不会自动反色。不同媒介的表面层次可以不同，比如网站浅色主题的浮层是白色，编辑器的侧栏是浅灰的纸色；背景、文字和强调色仍然用同一套参数。

## 字体、尺寸与交互

中英文正文都用无衬线字体，依次是 Inter、系统字体、Noto Sans SC 和 Microsoft YaHei。日期、编号和代码用 Cascadia Code 或系统等宽字体，公式用数学字体。读者没装这些字体也没关系，会用后面的回退字体。

页面容器最宽 1040 px；独立的阅读示例约 720 px；博客正文栏最宽 700 px，旁边留给目录。正文一般 16–18 px，基础行高 1.8，文章行高 1.95；标题随内容和视口缩放；图注约 12–13 px。宽度和行高在 `layout` 里改，切换主题不会改变正文尺寸。

间距用 8 / 12 / 16 / 24 / 32 / 48 / 64 px 这几档。精确图形用 0.6 pt 细线、1 pt 主线和 10–11 pt 标签，图内边距约 8%。示意图留出 25–35% 的空白，内容多的时候可以少留。

一般的交互反馈约 160 ms。图谱的出现、聚焦和移动可以慢一些，但动画只用来说明状态变化，并且遵守 `prefers-reduced-motion`。键盘焦点要看得见，按钮要有明确的可访问名称。窄屏上可以换行、重排，代码块可以单独横向滚动，但正文页面不能横向溢出。首页的光晕和扫描线只放在静态背景层，正文上不加纹理。

开启高对比度时，次要文字和分隔线的对比度会提高；打印时用白纸黑字。这两种情况的参数也写在下面的 JSON 里。

## 用在哪里

| 用在 | 共用 | 各自保留 |
| --- | --- | --- |
| 博客 | 背景、文字、强调色、字体、基础尺寸和间距 | 首页、系列、图谱、关于、搜索和阅读功能原有的结构 |
| VS Code | 两套主题的基础色和语义色 | 语法、诊断、Git 和 ANSI 颜色之间的区分；不改用户的字体设置 |
| Typst | 线宽、标签、配色和对齐规则 | 作者已有的变量、导入、几何、方程和箭头方向；尽量沿用现有代码 |
| AI 配图 | 色调、清楚的主体、留白、准确的关系 | 按内容自由构图，必要时可以有少量标签；默认不套用旧的抽象参考图 |

AI 配图分三种用途：`editorial` 是正文配图，`concept` 画关系和对照，`cover` 是封面，给另外排版的标题留出位置。精确的数值、算法流程和公式请用 Typst / SVG 这类可以核对的方式画。生成的图片要检查关系、标注、方向，以及缩小后能不能看清；示意用的坐标不能当成测量结果。

可复用的精确图形模板在 `design/visual-system/typst/blog-diagram.typ`。以前的配图实验可以留在本地，不会进入新建站点的模板。同步参数不会重画已有的图片、作者自己画的图和私人简历。正式网站只需要生成好的 CSS；安装后的插件用的是打包时的数据，运行时不读取仓库里的文件。

## 参数

实际参数只写在这里。`schemaVersion` 是读取格式的版本，`name` 和 `version` 是你的风格名称和版本号，`editor` 里是编辑器专用的表面色、语义色和状态色。

<!-- zen:tokens -->
```json
{
  "schemaVersion": 1,
  "name": "ZEN",
  "version": "1.0.0",
  "fonts": {
    "sans": ["Inter", "-apple-system", "BlinkMacSystemFont", "Segoe UI", "Noto Sans SC", "Microsoft YaHei", "sans-serif"],
    "mono": ["Cascadia Code", "SFMono-Regular", "Consolas", "Liberation Mono", "monospace"],
    "serif": ["Georgia", "Cambria", "Times New Roman", "serif"],
    "figure": ["Arial", "Microsoft YaHei"]
  },
  "layout": {"pageWidth": 1040, "readingWidth": 720, "bodyLineHeight": 1.8, "articleWidth": 700, "articleLineHeight": 1.95},
  "spacing": [8, 12, 16, 24, 32, 48, 64],
  "radius": {"small": 3, "medium": 6, "large": 8},
  "motion": {"feedbackMs": 160},
  "figure": {"finePt": 0.6, "strongPt": 1, "labelPt": 10, "marginPt": 20},
  "print": {"background": "#ffffff", "surface": "#ffffff", "raised": "#eeeeee", "border": "#cccccc", "ink": "#000000", "secondary": "#333333", "muted": "#444444", "accent": "#000000"},
  "palettes": {
    "graphite": {
      "background": "#121213", "surface": "#171719", "raised": "#1e1e21", "border": "#2c2c30", "edge": "#424247",
      "ink": "#ededee", "secondary": "#aaaab2", "muted": "#92929b", "accent": "#de8b82", "accentMuted": "#babac3",
      "contrast": {"secondary": "#d6d6dc", "muted": "#bcbcc6", "border": "#666670"},
      "imageWords": "flat near-black graphite background, ivory-white fine ink lines, restrained cool gray, one muted terracotta-red accent",
      "effects": {"halo": "rgb(207 211 225 / .07)", "scanline": "rgb(229 230 238 / .055)", "nav": "rgb(18 18 19 / .88)", "edgeLight": "rgb(255 255 255 / .035)", "shadow": "0 16px 48px rgb(0 0 0 / .12)"},
      "editor": {
        "panel": "#171719", "raised": "#1e1e21", "string": "#b4bda5", "typeColor": "#adbccd", "number": "#c9b397", "function": "#ddd1be", "purple": "#beb0c8",
        "error": "#e99a93", "warning": "#d6bb8c", "success": "#a8be9b", "info": "#a7bdd0",
        "selection": "#2f292d", "inactiveSelection": "#2b272d", "find": "#342725", "line": "#1a1a1d", "buttonFg": "#121213", "buttonHover": "#e79c94", "shadow": "#00000050",
        "ansiBlack": "#6f6f79", "ansiWhite": "#d7d7dc", "ansiCyan": "#9dbfbd", "ansiBrightCyan": "#b1d0ce"
      }
    },
    "paper": {
      "background": "#f8f7f4", "surface": "#ffffff", "raised": "#efeeeb", "border": "#deddd8", "edge": "#bcbcc4",
      "ink": "#242427", "secondary": "#5e5e68", "muted": "#666670", "accent": "#9d4941", "accentMuted": "#5e5e68",
      "contrast": {"secondary": "#33333b", "muted": "#454550", "border": "#90909a"},
      "imageWords": "flat warm off-white background, charcoal-black fine ink lines, restrained gray, one dark terracotta-red accent",
      "effects": {"halo": "rgb(76 79 95 / .04)", "scanline": "rgb(56 56 70 / .035)", "nav": "rgb(248 247 244 / .88)", "edgeLight": "rgb(255 255 255 / .65)", "shadow": "0 16px 48px rgb(45 41 35 / .045)"},
      "editor": {
        "panel": "#efeeeb", "raised": "#ffffff", "string": "#4c6248", "typeColor": "#456078", "number": "#78572f", "function": "#65513a", "purple": "#745c80",
        "error": "#a63432", "warning": "#80591f", "success": "#48663d", "info": "#3f627f",
        "selection": "#eee5df", "inactiveSelection": "#eae6e1", "find": "#efe4da", "line": "#f0eeea", "buttonFg": "#ffffff", "buttonHover": "#883e37", "shadow": "#24242718",
        "ansiBlack": "#33333b", "ansiWhite": "#74747d", "ansiCyan": "#376967", "ansiBrightCyan": "#376967"
      }
    }
  }
}
```
<!-- /zen:tokens -->

## AI 共用规则

下面的英文规则会直接写进插件的提示词，色值和 Typst 尺寸从上面的参数里填入。规则里保留了必要的科学约束，构图交给每张图自己决定。

<!-- zen:prompts -->
```json
{
  "image": {
    "opening": "Draw a clear, appealing explanatory illustration for a research blog using the ZEN design language. Meaning and readability come first.",
    "treatment": "a small amount of another muted color is welcome if it clarifies categories. Use confident clean contours, simple forms, subtle shading where helpful, and generous negative space.",
    "constraints": "Use only a few short, legible labels when needed. No article title, paragraphs, bullet lists, page furniture or four-panel lecture layout unless explicitly requested. Do not force engraved contour fields, abstract geometric decoration or a fixed diagram template. Preserve the source relationships and distinguish illustrative coordinates from real measurements. No invented results, pseudo-text or watermarks.",
    "compositions": {
      "editorial": "Create an engaging article illustration whose recognizable objects or schematic elements make the main idea intuitive. Give the composition room to breathe; choose the layout to suit the subject.",
      "cover": "Create an expressive topic illustration with a strong visual focus and quiet space for a separately typeset title. Choose a natural composition for the topic.",
      "concept": "Create an intuitive schematic showing the core relationship or contrast. Use recognizable subject drawings, vector cells, groups or arrows when they clarify the idea. Group comparisons naturally, not as a slide deck. Do not imply a sequential pipeline unless the source describes one."
    }
  },
  "typst": {
    "opening": "For Typst figure code, use the blog's ZEN restrained experimental-publication visual system when the surrounding code does not already define a style. Preserve existing palette variables, imports and layout.",
    "constraints": "Use named color variables, readable sans-serif labels and monospaced identifiers, generous spacing and aligned geometry. Use the configured accent sparingly for the key relation; no ornamental cards, gradients or decorative scientific marks. Exact counts, proportions, equations and arrow directions must follow the source. Render labels with Typst text, not raster assets. Prefer native Typst elements or the CeTZ version already imported; never invent an import or dependency version. Keep the cursor completion small: do not add page settings, imports or a full template in the middle of an expression. The source can use a blogdiagram starter with bg, ink, muted and accent variables; reuse them when present."
  }
}
```
<!-- /zen:prompts -->

## 维护与验证

```powershell
# 改完 ZEN 后同步所有生成文件
blogctl zen
# 只检查生成文件是否已同步，不写入
blogctl zen --check
# 照常构建网站
blogctl build
```

生成的文件有 `site/static/zen.css` 和 Typst 模板里的 ZEN 参数段。在 Research Blog 的源码仓库里，还会一起更新 `design/zen.generated.cjs`、两套主题 JSON、主题预览页和 `blogdiagram` 片段。这些生成内容不要手改。`apollo.css` 和各页面的样式只管组件和布局，用的是 `zen.css` 提供的变量。

新组件先用现有的语义变量，确实需要新的语义角色时再加到这个文件里。改完之后检查深色和浅色主题、手机上的排版、键盘操作，以及图形的方向和可读性；构建时会检查生成文件有没有同步。改了本地的 ZEN 不会自动更新线上网站，也不会替换已经安装的 VSIX，要按正常流程发布或重新安装。
