+++
title = "公式、双链与 Typst 图形（演示）"
date = 2026-01-02
description = "演示带下标与星号的 Markdown 公式免转义、章节锚点双链和 Typst 流程图。"
draft = false
[taxonomies]
tags = ["writing", "typst"]
+++

这是一篇演示文章，展示模板站点里的公式保护、章节双链和 Typst 矢量图。

## 免转义的 Markdown 公式

写推导时经常出现下划线 `_` 和星号 `*`。普通静态博客容易把它们误判成斜体，这里直接写原文即可：

- 行内下标与最优解：$x_1, x_2, \dots, x_n$ 与 $w_i^*, \theta^*$。
- 行内普通文本里的字面美元符号写成 `\$100` 和 `\$200`，不会触发公式渲染。
- 多行对齐与矩阵环境无需额外转义：

$$
\begin{aligned}
L(\theta) &= \frac{1}{N} \sum_{i=1}^{N} \ell(f_\theta(x_i), y_i) + \lambda \|\theta\|_2^2 \\
\theta^* &= \arg\min_{\theta} L(\theta)
\end{aligned}
$$

## 章节双链与关系图谱

除了 [[开始你的研究笔记]] 和 [[风格与写作]] 这样的整篇双链，还可以直接连到某篇文章的具体章节，例如 [[开始你的研究笔记#用 Typst 画的图|第一篇里的 Typst 图形示例]]。

只要一篇文章链接了另一篇，对方底部就会自动列出反向链接，顶部「图谱」页面也会把它们连成网络。文献引用仍然使用 `[@citekey]`，例如 [@zotero-guide]。

## 编译为 SVG 的流程图

下面的示意图来自同目录的 `figures/workflow-pipeline.typ`，修改源码保存后会在预览里自动重新编译：

{{ <fig id="workflow-pipeline" caption="演示：从 Markdown、Typst 与 ZEN.md 到静态网站的构建流程" alt="三步流程：Markdown 与 Typst 源文件经过 blogctl build 输出 Zola 静态站点" /> }}
