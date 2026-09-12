+++
title = "从轨迹中提取可复用技能"
date = "2024-09-01"
slug = "reusable-skills"
description = "探讨如何通过行为等价关系从原始轨迹数据中抽象出可复用的技能表示"
[taxonomies]
tags = ["reinforcement-learning", "skill-discovery", "behavioral-equivalence"]
[extra]
author = "Researcher"
+++

# 引言

在强化学习中，智能体通常通过大量试错收集原始轨迹数据。然而，原始轨迹往往包含大量冗余和噪声，直接用于后续任务效率低下。本文探讨如何通过**行为等价**的视角，将轨迹抽象为可复用的技能表示。

核心思想：两个状态-动作序列如果在所有可能的后续任务中产生相同的效果，则它们在行为上等价，可以被映射到同一个抽象技能。

## 行为等价与技能商空间

### 等价关系的定义

给定一个 MDP $\mathcal{M} = (S, A, P, R, \gamma)$ 和轨迹集合 $\mathcal{D}$，我们定义两个子轨迹 $\tau_1, \tau_2$ 行为等价，当且仅当：

<div class="math-block">
$$
\forall s \in S: Q^*_{\mathcal{M}}(s, \tau_1) = Q^*_{\mathcal{M}}(s, \tau_2)
$$
</div>

这里 $Q^*$ 是最优 Q 函数，$\tau$ 被视为从状态 $s$ 开始执行的宏动作。

### 商空间构造

通过这个等价关系，我们可以将原始轨迹空间 $\mathcal{T}$ 划分为等价类：

<div class="math-block">
$$
[\tau] = \{ \tau' \in \mathcal{T} : \tau' \sim \tau \}
$$
</div>

商空间 $\mathcal{T} / \sim$ 即为技能空间，每个等价类对应一个可复用的抽象技能。

{{ <fig id="quotient-space" caption="技能商空间示意图" alt="从原始轨迹空间到技能商空间的投影" /> }}

### 为什么要求对组合封闭

等价关系必须对**顺序组合**封闭，即如果 $\tau_1 \sim \tau_1'$ 且 $\tau_2 \sim \tau_2'$，则：

<div class="math-block">
$$
\tau_1; \tau_2 \sim \tau_1'; \tau_2'
$$
</div>

这保证了学到的技能可以自由组合而不破坏行为语义。不满足此性质的"伪技能"在实际使用中会失效。

## 实现方法

### 轨迹聚类

```python
def cluster_trajectories(trajectories, distance_fn, threshold):
    """将轨迹聚类为技能等价类"""
    clusters = []
    for traj in trajectories:
        matched = False
        for cluster in clusters:
            if distance_fn(traj, cluster.representative) < threshold:
                cluster.add(traj)
                matched = True
                break
        if not matched:
            clusters.append(Cluster(traj))
    return clusters
```

实际实现中，我们用基于后继状态分布的度量函数近似行为等价：

<div class="math-block">
$$
d(\tau_1, \tau_2) = \mathbb{E}_{s \sim \rho_0} \| \phi(s_{\tau_1}) - \phi(s_{\tau_2}) \|
$$
</div>

其中 $\phi$ 是学习到的状态表示，$s_{\tau}$ 是执行轨迹 $\tau$ 后到达的状态。

## 讨论

通过行为等价关系构造技能商空间，能够从原始轨迹中自动提取出真正可复用、可组合的抽象技能表示。这为分层强化学习和知识迁移提供了理论基础。

未来工作包括：在部分可观测环境中扩展该框架，以及研究非马尔可夫技能的表示。

*注：本文为方法论探讨，不包含实验结果。*
