// ZEN 精确图形：可复制到文章 figures/ 目录的原生 Typst 模板。
// 深色导出：typst compile --input theme=graphite blog-diagram.typ diagram.svg
// zen:begin — generated from ZEN.md; keep this template standalone.
#let dark = sys.inputs.at("theme", default: "paper") == "graphite"
#let bg = if dark { rgb("#121213") } else { rgb("#f8f7f4") }
#let ink = if dark { rgb("#ededee") } else { rgb("#242427") }
#let muted = if dark { rgb("#aaaab2") } else { rgb("#5e5e68") }
#let accent = if dark { rgb("#de8b82") } else { rgb("#9d4941") }
#let fine = 0.6pt
#let strong = 1pt
#set page(width: auto, height: auto, margin: 20pt, fill: bg)
#set text(font: ("Arial", "Microsoft YaHei"), size: 10pt, fill: ink)
// zen:end
#set par(leading: 0.7em)

#let node(body, emphasis: false) = box(
  inset: 12pt,
  stroke: (paint: if emphasis { accent } else { muted }, thickness: if emphasis { strong } else { fine }),
  body,
)
#let cell(value, active: false) = box(width: 25pt, height: 25pt,
  stroke: (paint: if active { accent } else { muted }, thickness: fine),
  align(center + horizon, text(fill: if active { accent } else { ink }, weight: if active { "bold" } else { "regular" })[#value]),
)

// 示例：数值、字形与对齐由代码确定。替换此处内容即可复用样式。
#stack(dir: ttb, spacing: 14pt,
  text(size: 12pt, weight: "bold")[One-hot · 每个词对应一个位置],
  text(fill: muted)[词表顺序：猫 / 狗 / 鱼],
  grid(columns: (38pt, auto, auto, auto), gutter: 0pt, row-gutter: 8pt, align: center + horizon,
    [猫], cell(1, active: true), cell(0), cell(0),
    [狗], cell(0), cell(1, active: true), cell(0),
    [鱼], cell(0), cell(0), cell(1, active: true),
  ),
  text(size: 9pt, fill: muted)[位置标识不同词，不直接表示语义相似性。],
)
