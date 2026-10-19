+++
title = "开始你的研究笔记"
date = 2026-01-01
description = "试试实时预览、公式、Typst 图形、双链和文献引用。"
draft = false
[taxonomies]
tags = ["writing"]
+++

这篇文章演示基本的写法。打开网站预览，改一下正文再保存，页面就会更新。装好写作插件后，也可以在 VS Code 里运行 **Research Blog: 预览网站（含草稿）** 打开同样的预览。

## 公式与代码

行内公式 $a^2+b^2=c^2$ 和独立公式都直接写在 Markdown 里：

$$
\mathbb{E}[X] = \sum_x x\,p(x)
$$

```python
def mean(values):
    return sum(values) / len(values)
```

## 用 Typst 画的图

下面这张图的源码是文章目录里的 `figures/style-example.typ`。它用了项目共享的 Typst 模板，模板的配色、线宽和字体由根目录的 `ZEN.md` 生成。

{{ <fig id="style-example" caption="示例词表的 one-hot 表示" alt="猫、狗、鱼分别对应三个向量位置" /> }}

这是示意图，不代表实验结果。风格文件的用法见 [[风格与写作|风格与写作]]。

## 引用文献

引用文献写成 `[@citekey]`。比如，Zotero 的入门指南介绍了怎样收集、整理和引用文献 [@zotero-guide]。

示例的书目数据在 `site/data/references.json`。连上自己的 Zotero 之后，从“文献工作台”插入引用，需要公开的书目信息会自动写进这个文件。PDF、密钥和私人批注都留在本地。
