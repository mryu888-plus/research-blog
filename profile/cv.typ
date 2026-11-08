// 编辑此文件，然后运行 cargo run -p blogctl -- resume。英文版在 cv.en.typ。
// 同一份数据生成博客个人介绍页与 PDF 简历；排版在 layout.typ 中维护。
// 字段使用字符串、数组和字典，支持 Typst 表达式与字符串插值。
// 电话只通过本地构建输入传入，不写入此文件或公开页面的数据。
#import "layout.typ": render-resume

#let current-research = "目前在 NTU DANTE 数据管理研究组，由 Cong Gao 教授指导，研究大规模 Agent 轨迹数据管理与复用，围绕统一数据库、语义层与物化优化开展系统建设和基准评测，目标投稿 ICML。"

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
  tagline: "智能体数据管理 · 技能图谱 · 智能体系统 · 生成式建模",
  education: (
    (
      degree: "人工智能理学硕士",
      school: "新加坡南洋理工大学",
      date: "2026.08 - 2027.08（预计）",
      detail: "人工智能硕士在读，研究方向为智能体数据管理与复用。",
    ),
    (
      degree: "高级计算荣誉学士（一等荣誉）｜Dalyell Scholar",
      school: "悉尼大学",
      date: "2021.08 - 2025.12",
      detail: "计算数据科学与数学双主修；荣誉研究导师为 BMIT 研究组 Xiuying Wang 副教授。",
    ),
  ),
  experiences: (
    (
      kind: "research",
      org: "南洋理工大学",
      role: "研究项目｜DANTE 数据管理研究组｜导师：Cong Gao 教授",
      date: "进行中",
      venue: "ICML",
      status: "目标投稿",
      paper_title: "大规模 Agent 轨迹数据管理与复用",
      bullets: (
        "负责面向大规模 Agent 轨迹的统一数据库设计与建设，支持执行数据的组织、查询与复用。",
        "设计面向智能体的语义层，关联任务上下文与历史执行经验，让 Agent 按任务检索可复用的经验。",
        "围绕数据物化与查询效率开展系统优化，负责基准评测方案设计与性能分析。",
      ),
      web_title: "大规模 Agent 轨迹数据管理与复用",
      summary: "在 NTU DANTE 数据管理研究组、Cong Gao 教授指导下，负责面向大规模 Agent 轨迹的统一数据库设计与建设，支持执行数据的组织、查询与复用；设计关联任务上下文与历史经验的语义层，并围绕数据物化与查询效率开展系统优化、基准评测方案设计及性能分析。目标投稿 ICML。",
    ),
    (
      kind: "internship",
      org: "阿里巴巴集团 · 淘天集团",
      role: "人工智能研发实习｜论文第一作者",
      date: "2026.05 - 2026.08",
      venue: "ICLR 2027",
      status: "已投稿",
      paper_title: "SkillWorld: World-Indexed Skill Graphs with Governed Self-Evolution",
      bullets: (
        "提出 SkillWorld：面向 LLM Agent 的技能图谱，按执行状态检索技能；完成全部代码和论文。",
        "部署于企业内部平台：903 项技能，36 项困难任务平均成功率 72.2%，比 SkillDAG 高 5.6 个点。",
        "SkillsBench 72 项任务成功率比各模型最强基线高 11.1-15.3 个百分点；ALFWorld 15 步内成功率 85.1%-85.8%，无效动作比 BM25 少 43.5%-72.4%。",
        "技能知识层用 Datalog / Soufflé 从技能描述和共享本体推导关系，按执行状态判断是否适用；本体按证据更新，经 Lean 4 校验后发布。",
      ),
      web_title: "SkillWorld · 智能体技能图谱",
      summary: "提出 SkillWorld 项目，作为主要贡献者完成全部代码实现与论文撰写。构建智能体技能知识层，从类型化技能描述与共享本体推导关系，按当前执行状态判断关系是否适用，并依据源证据更新本体与图谱。系统已部署于企业内部服务平台，在 903 项技能支持的 36 项困难任务上取得 72.2% 平均成功率（三次运行）。",
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
        "提出异质性自适应扩散薛定谔桥，结合 PET 引导和区域条件语义嵌入做全身 MRI 模态转换。",
        "平均 SSIM 91.1%、PSNR 26.25 dB，比最强基线高 4.0 个百分点和 1.88 dB（PyTorch 实现）。",
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
      venue: "期刊论文",
      status: "撰写中",
      paper_title: "面向 SGM 算法的 AI 辅助 ROI 时序建模",
      bullets: (
        "为 SGM 算法构建 AI 辅助模块，将最优时间窗选择建模为变长 ROI 滞后曲线集合上的时序学习问题。",
        "设计置换不变的区间回归与候选排序模型筛选稳定窗口，并用可靠性掩码抑制伪影时间点。",
      ),
      web_title: "面向 SGM 算法的 AI 辅助 ROI 时序建模",
      summary: "为 SGM 算法构建 AI 辅助模块，将最优时间窗选择建模为变长 ROI 滞后曲线集合上的时序学习问题。",
    ),
  ),
  honors: (
    (year: "2024", title: "SYNC’S Hackathon 第三名"),
    (year: "2024", title: "Coding Fest 亚军"),
    (year: "2022", title: "ICM 成功参赛奖"),
  ),
)

// 技能只出现在 PDF 简历里，不进入网站个人页的数据。
#let skills = (
  (label: "智能体", items: "技能图谱、经验复用、RAG、Agent 评测（SkillsBench、ALFWorld）、Claude Code / Codex"),
  (label: "编程与工具", items: "Python、SQL、Rust、PyTorch；Docker、Linux、Git"),
  (label: "知识与数据", items: "本体建模、知识图谱、Datalog / Soufflé、Lean 4；数据库设计、查询与物化优化"),
  (label: "语言", items: "中文（母语）、英文（流利）"),
)

// 导出器读取 Typst 实际求值后的数据，不解析源码文本。
#metadata(profile) <profile>
#render-resume(profile, skills: skills, phone: sys.inputs.at("phone", default: ""))
