// 演示图形：基于 ZEN 规范模板，绘制写作与构建流程示意图。
#let dark = sys.inputs.at("theme", default: "paper") == "graphite"
#let bg = if dark { rgb("#121213") } else { rgb("#f8f7f4") }
#let ink = if dark { rgb("#ededee") } else { rgb("#242427") }
#let muted = if dark { rgb("#aaaab2") } else { rgb("#5e5e68") }
#let accent = if dark { rgb("#de8b82") } else { rgb("#9d4941") }
#let fine = 0.6pt
#let strong = 1pt
#set page(width: auto, height: auto, margin: 20pt, fill: bg)
#set text(font: ("Arial", "Microsoft YaHei"), size: 10pt, fill: ink)
#set par(leading: 0.7em)

#let stage(title, subtitle, emphasis: false) = box(
  width: 118pt,
  inset: 10pt,
  radius: 3pt,
  stroke: (paint: if emphasis { accent } else { muted }, thickness: if emphasis { strong } else { fine }),
  stack(dir: ttb, spacing: 5pt,
    text(weight: "bold", fill: if emphasis { accent } else { ink })[#title],
    text(size: 8.5pt, fill: muted)[#subtitle],
  ),
)

#let arrow() = align(center + horizon, text(fill: muted)[->])

#stack(dir: ttb, spacing: 12pt,
  text(size: 11pt, weight: "bold")[演示流程 · 从源文件到静态站点],
  grid(
    columns: (auto, 22pt, auto, 22pt, auto),
    align: center + horizon,
    stage("Markdown + Typst", "公式原文 / 矢量源码 / ZEN"),
    arrow(),
    stage("blogctl build", "保护公式 · 编译图 · 排版引用", emphasis: true),
    arrow(),
    stage("Zola 静态站点", "页面 · 图谱 · 全文检索 · RSS"),
  ),
  text(size: 8.5pt, fill: muted)[演示示意图：构建时编译为同目录 SVG，正文可点击放大。],
)
