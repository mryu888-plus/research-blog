// 编辑此文件，然后运行 python scripts/editor.py resume。
// 同一份数据生成博客个人介绍页与 PDF 简历；排版在 layout.typ 中维护。
// 字段使用字符串、数组和字典，支持 Typst 表达式与字符串插值。
// 电话只通过本地构建输入传入，不写入此文件或公开页面的数据。
#import "layout.typ": render-resume

#let current-research = "目前在 NTU 的 Cong Gao 教授指导下，从事轨迹抽取与知识复用研究。"

#let profile = (
  schema_version: 1,
  display_name: "于嘉澄",
  english_name: "Jiacheng Yu",
  intro: "我是于嘉澄，在南洋理工大学读人工智能硕士。",
  current_research: current-research,
  research_interests: "目前的研究兴趣包括 Agent、技能图谱、DB for AI，以及知识抽取与表示。",
  github: "https://github.com/mryu888-plus",
  email: "aolianjijuzi@gmail.com",
  location: "中国 / 新加坡",
  tagline: "轨迹抽取 · 知识复用 · 智能体系统 · 生成式建模",
  education: (
    (
      degree: "人工智能理学硕士",
      school: "新加坡南洋理工大学",
      date: "2026.08 - 2027.08（预计）",
      detail: "人工智能硕士在读。" + current-research,
    ),
    (
      degree: "高级计算荣誉学士（一等荣誉）｜Dalyell Scholar",
      school: "悉尼大学",
      date: "2021.08 - 2025.12",
      detail: "计算数据科学与数学双主修；核心课程涵盖机器学习、计算机视觉、算法设计、抽象代数与代数逻辑。荣誉研究聚焦 PET 引导的全身 MRI 模态转换与扩散生成模型。",
    ),
  ),
  experiences: (
    (
      kind: "internship",
      org: "阿里巴巴集团 · 淘天集团",
      role: "人工智能研发实习｜论文第一作者",
      date: "2026.05 - 2026.08",
      venue: "ICLR 2027",
      status: "待投稿",
      paper_title: "SkillWorld: World-Indexed Skill Graphs with Governed Self-Evolution",
      bullets: (
        "设计类型化 SkillManifest，统一描述能力、输入/输出端口、前置条件与效果；由 Datalog 规则编译带推导证据的不可变潜在图。",
        "实现确定性的 Strong-Kleene 世界状态投影，输出 confirmed / unknown / rejected；以证据门控和 Lean 4 约束演化、修复与发布。",
        "在 SkillsBench 静态构建、134 项 ALFWorld 状态条件执行及 500 项 Skill 工业语料评测中取得当前 SOTA 表现。",
      ),
      web_title: "SkillWorld · 智能体技能图谱",
      summary: "用图谱组织 Agent 的技能，描述每项技能的输入输出、前置条件和效果。根据当前状态判断技能是否可用，并通过规则和证据约束技能的更新。",
    ),
    (
      kind: "research",
      org: "悉尼大学 - 上海交通大学联合研究中心",
      role: "荣誉研究｜共同第一作者（Equal Contribution）",
      date: "2025.01 - 2026.01",
      venue: "MICCAI 2026",
      status: "接收",
      paper_title: "Heterogeneity-Adaptive Diffusion Schrödinger Bridge for PET-Guided Whole-Body MRI Translation",
      bullets: (
        "提出异质性自适应扩散薛定谔桥，整合 PET 引导与端点桥过程建模，实现全身 MRI 模态转换。",
        "引入区域条件语义嵌入，并融合 PET 与辅助影像序列，增强跨解剖区域的结构一致性与细节保真度。",
        "搭建可复现的 PyTorch 训练评估管线，在 PET 引导的全身 MRI 模态转换任务上取得当前 SOTA 表现。",
      ),
      web_title: "跨模态生成",
      summary: "全身 MRI 模态转换，使用扩散薛定谔桥处理不同解剖区域的差异，改善生成图像的结构一致性。",
      paper_url: "https://arxiv.org/abs/2607.07401",
    ),
    (
      kind: "research",
      org: "悉尼大学｜跨课题组合作",
      role: "交叉学科研究",
      date: "2026 - 至今",
      venue: "PNAS",
      status: "撰写中",
      paper_title: "面向 SGM 算法的 AI 辅助 ROI 时序建模",
      bullets: (
        "为 SGM 算法构建 AI 辅助模块，将最优时间窗选择建模为变长 ROI 滞后曲线集合上的时序学习问题。",
        "设计置换不变的区间回归与候选区间排序模型，为 SGM 自动筛选稳定、可信的分析窗口。",
        "引入可靠性掩码抑制伪影污染时间点，在保留真实多尺度动态的同时提升 SGM 输出稳健性。",
      ),
      web_title: "面向 SGM 算法的 AI 辅助 ROI 时序建模",
      summary: "为 SGM 算法构建 AI 辅助模块，将最优时间窗选择建模为变长 ROI 滞后曲线集合上的时序学习问题。",
    ),
  ),
  honors: (
    (year: "2024", title: "SYNC’S Hackathon 第三名"),
    (year: "2024", title: "Coding Fest 亚军"),
    (year: "2022", title: "ICM SP"),
  ),
)

// 导出器读取 Typst 实际求值后的数据，不解析源码文本。
#metadata(profile) <profile>
#render-resume(profile, phone: sys.inputs.at("phone", default: ""))
