# ZEN · 视觉预览与 Typst 图形

完整设计语言与唯一参数源见根目录 [ZEN.md](../../ZEN.md)。博客、VS Code 配色、Typst 模板及 AI 配图共用 ZEN：石墨黑、暖纸白、灰阶、少量陶土红与清楚的结构。

本目录保留视觉探索和阅读情境预览。`index.html` 可切换明暗主题，样式通过 `site/static/zen.css` 使用共同参数；早期抽象线场、轮廓与切片样图作为探索档案，不再充当默认 AI 构图规则。

## 精确图形

`typst/blog-diagram.typ` 是可独立复制到文章 `figures/` 目录的原生 Typst 模板。在 VS Code 空白 `.typ` 文件输入 `blogdiagram` 可插入同一份模板。模板的 ZEN 参数段、片段和 Typst AI 补全规则均由根目录规范生成。

```powershell
node scripts/sync_zen.cjs
typst compile design/visual-system/typst/blog-diagram.typ diagram.svg
typst compile --input theme=graphite design/visual-system/typst/blog-diagram.typ diagram-dark.svg
```

模板使用两档线宽、清楚的标签、精确对齐与克制的强调色。AI 补全优先保留已有变量、导入和布局；数量、方程、比例与箭头方向必须服从原文。复制到文章后的图形属于作者内容，不会被参数同步覆盖。

## AI 配图

AI 配图根据选区直接生成正文或概念示意图，采用自然构图；`editorial`、`concept`、`cover` 分别对应主题配图、关系对照和系列封面。默认不启用旧抽象参考图，也不调用文字模型预处理。色调与共用约束来自 ZEN，具体布局服从主题；必要时可以保留少量清楚的标签。

精确算法图、实验数据和公式使用可校验的图形代码。AI 图片应检查关系、标注、方向及缩小后的可读性；不要把示意线条当作测量结果，不自动反色或染色已有位图。

## 探索档案与来源

- `assets/semantic-field.png`、`equivalence-plate.png`、`knowledge-strata-v2.png` 是早期三种用途的探索样图，由已接入的 Qwen Image 3.0 Pro 经百炼 Token Plan 生成。
- 图片旁的 `.prompt.txt` 保存当时的提示词。早期 `knowledge-strata.png` 出现伪文字，保留为未采用稿。
- `examples/nlp-representations/` 保留 NLP 配图探索及相应提示词。
- `extensions/research-blog/assets/editorial-reference.png` 是可选旧参考图；用户显式开启后才用于支持参考图的接口。

改风格时编辑根目录 ZEN，再运行同步；不要分别修改生成色值或提示词。旧图片及其来源记录保留原样。
