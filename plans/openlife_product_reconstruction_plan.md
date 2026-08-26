# OpenLife 产品重建计划

状态：唯一活动计划
起点：2026-08-24 产品现实审计
目标：让 OpenLife 达到一线桌面 Agent 的正常使用体验，而不是继续累积局部能力

## 成功定义

只有同时满足下列条件，才能称为“可用”：

- 12 条核心旅程全部在用户实际启动的正式签名应用中通过；
- 正式应用自报提交、Bundle ID、签名、二进制摘要和已安装路径与验收构建一致；
- 没有开放 P0/P1；
- 每个完成声明都有对应目标、工具/来源/Artifact 回执和重启后持久化证据；
- 不存在“没有完成用户目标却显示完成”的错误成功；
- UI、键盘、VoiceOver、200% 缩放、错误恢复和典型窗口尺寸达到正式产品质量；
- 新链路通过后，同一能力的旧组件、旧 IPC、旧运行路径和兼容 fallback 被删除。

## 产品边界

本计划范围：

- 单一 Conversation/composer 中的 Chat 与 Work；
- 持久化 Provider/Model Profile；
- 真实本地文件夹 Project；
- 文件读取、搜索、编辑、Artifact、预览、diff、撤销；
- 任务进度、steering、stop/resume、JIT Review、失败恢复；
- 历史；
- 可选 Agent Memory/LifeModel；
- 公共 Web 研究与引用；
- 独立文件、URL 和资源。

冻结：

- Computer Use 产品能力；
- connectors/plugin 大平台；
- multi-agent 产品化；
- schedules/automation；
- 新的治理平台、问题台账、task packet、append-only evidence registry；
- 登录/账户体系；
- 在产品可用前做差异化创新。

## 工作方式

- 一次只允许一个改变生产行为的纵向切片。
- 可并行的只有只读研究、设计审阅和独立 QA。
- 每个切片从用户正式入口开始，经过前端、IPC、Rust runtime、SQLite/文件系统，再回到正式 UI。
- 单元/组件/浏览器壳测试是中间证据；正式签名安装包是交付事实。
- 不建立新平台来管理计划。本文件就是唯一活动计划。
- 旧代码在替代链路完成正式验收后立即删除，不保留长期 V1/V2 双轨。

## Gate A — 产品与交互契约重建

在可点击产品原型前先冻结以下产品契约：

### Provider/Profile

- Profile 是持久化实体，不是运行时 ViewModel 列表。
- 一个 Profile 绑定 provider、endpoint、credential reference、可用模型、默认模型、协议、能力、验证状态和隐私边界。
- 支持多个云端、自定义兼容端点和本地 Ollama Profile。
- composer 选择“本轮 Profile + model + reasoning”；Settings 管理 Profile 生命周期。
- 任何凭据只显示遮罩，不进入日志、截图或诊断。

### Project/Folder

- “打开文件夹”创建或选择 Project，并绑定真实目录权限。
- 无 Project 的新对话默认 Chat；从文件夹入口开始的新对话默认 Work。
- Project 必须支持目录枚举、文件名/内容搜索、读取、多文件选择、创建、修改、重命名、预览、diff、撤销和重启恢复。
- 额外读取目录是清晰的只读范围，不与主 Project 写范围混淆。

### Conversation/Task/Result

- 左侧：Projects、Conversations、History，避免 Activity 与 Task 重复堆叠。
- 中央：Thread、composer、进行中的简明步骤和 steering。
- 右侧上下文面板：仅在需要时显示 preview/diff/review/result/details。
- 错误必须给出根因、影响和直接恢复动作；技术字段折叠在二级详情。
- Memory/LifeModel 是可选增强，不得成为普通文件工作前提。

退出条件：契约进入稳定 ADR/架构文档；没有新治理层；所有实现切片都能引用同一组产品状态与 backend ViewModel。

## Gate B — 仓库内可点击产品原型

不依赖 Figma、付费设计工具或新的设计平台。原型放在
`docs/prototypes/openlife-product-experience/`，使用独立静态 HTML、CSS 和
JavaScript，直接导入生产 `openlife.tokens.css`。原型只模拟产品状态，不连接
IPC、SQLite、凭据、真实 Provider、真实文件或外部服务，也不进入生产构建。

必须交付：

- foundations：色彩、字体、间距、圆角、层级、focus、状态；
- components：shell、project/conversation rows、composer、model/profile picker、task step、result、preview、diff、review、error/recovery、empty/loading/offline；
- 12 条旅程的关键 screen、状态切换与可点击原型；
- 常见桌面窗口、窄窗口、200% 缩放状态；
- light/dark 如当前产品范围需要；
- Gate C 冻结后的关键 screen 截图导出到仓库，避免把评审中间态固化为基线；
  设计 token/行为契约落在代码可消费文档中；
- 原型可通过仓库根目录下的静态服务器直接预览，不新增包管理器、运行时或
  独立应用框架；
- 交互由确定性的 fixture 和小型状态机驱动，所有模拟内容显式标注为设计
  原型，不能作为运行时、Provider、文件或正式原生证据。

设计方向：可信、平静、成熟；参考 ChatGPT Desktop/Codex 和 Claude Cowork 的产品结构，参考 Cursor 的 diff/recovery，但不复制品牌。

### 2026-08-24 已停止的 Figma 尝试

- Draft：`https://www.figma.com/design/LREH6agDjKQUdPLuSB4Gi8`
- Phase 0 已完成：代码 token、空白文件、可用库和 v1 范围已核对；
- 已创建 4 个单模式 collection、25 个 primitive color、25 个 semantic
  color alias、23 个 spacing/geometry 变量；
- Starter 计划在继续创建 Typography 前触发 Figma MCP 调用上限；
- 未完成：Typography、text/effect styles、页面、组件、12 条旅程、原型连线和导出验证；
- 用户明确选择不为 Figma 付费。该 Draft 仅保留为中止实验记录，不再是当前
  Gate、实现依赖或恢复目标；
- 本地原型必须达到与原 Figma 计划相同的旅程、状态、响应式、可访问性和
  用户确认标准，不能降级为静态线框图。

### 原型结构

```text
docs/prototypes/openlife-product-experience/
├── index.html          # 语义结构与可直接打开的入口
├── styles.css          # shell、组件、响应式与可访问状态
├── app.js              # 小型确定性状态机与交互
├── fixtures.js         # 12 条旅程的真实感模拟数据
├── README.md           # 预览、边界与验收说明
└── screenshots/        # 通过视觉 QA 的关键状态导出
```

不安装 Storybook，不创建第二个 React/Vite 工程，不复用生产 IPC，不把原型
组件伪装成已实现的产品组件。若原型中的模式被用户接受，生产切片再按真实
ViewModel 和运行路径重建。

### 2026-08-24 当前原型状态

- foundations、桌面 shell、composer、Profile、Project、progress、result、
  diff、review、error/recovery 和 contextual inspector 已实现；
- 12 条旅程各有 3 个确定性状态，共 36 个状态，已逐一完成浏览器交互检查；
- 已检查 1440×900、1024×768 和 720×450 CSS 视口；后者作为 1440×900
  在 200% 缩放时的布局等效检查，不替代正式应用的原生 200% 缩放验收；
- 已修复工作区最小高度导致 composer 与 Settings 被视口裁切的问题；
- JavaScript 语法、页面 console、语义 DOM、键盘旅程切换、Escape 收起和
  文档视口溢出检查已通过；
- 当前进入 Gate C 用户逐旅程走查。生产 UI、IPC 与 runtime 仍保持冻结。

### 2026-08-24 Gate C 首轮反馈收敛

- 用户判定首版展示过多、过于复杂，要求直接遵循一线 Agent 的成熟模式；
- 默认界面已收敛为轻量 sidebar、单一 conversation、底部 composer；
- Projects 与 Recents 保留在 sidebar，History、Memory、诊断和验收说明不再作为
  常驻一级导航；
- progress 默认只显示当前步骤，diff、Review、来源、失败详情和完整步骤按需
  打开 contextual inspector；
- 原型旅程控制已从产品界面移除，使用 `Alt+P` 打开内部走查抽屉；
- 12 条旅程和 36 个状态仍保留，简化展示不能以丢失失败、权限和恢复状态为
  代价。
- P0 交互已补齐：composer 内可选择 Profile/model/reasoning；“打开文件夹”覆盖
  可用、待授权、路径失效和系统选择器交接；多文件变更必须完成逐文件逐行审查
  后才能应用；Project、Conversation 与 New Chat 的 active state 与旅程同步；
- 上述 P0 已在桌面与 720×450、560×450 窄视口完成关键路径回归；生产 UI、IPC
  与 runtime 未修改。

## Gate C — 用户设计确认

- 通过本地预览逐条走查 12 条可点击旅程，而不是只批准静态首页；
- 确认 shell、Project、composer、Profile/model、progress、preview/diff/review、error/recovery 的关键状态；
- 记录必须修改的设计问题并回到 Gate B 收敛；
- 用户明确接受后冻结第一轮设计基线。

硬门：Gate C 通过前不修改生产 UI 或运行行为，不以“先做一点后端”绕过设计顺序。

## Slice 0 — 安全语义与正式发布完整性

目标：先消灭错误成功，再保证以后不出现“源码修了，用户正式应用没修”。

必须完成：

- 为 2026-08-24 误路由建立原生可复现测试和 runtime regression：读取 Project 文件不能转成 Memory；
- Personal Intelligence 写入必须有独立、可验证的用户意图证明；`sourceSpan` 只证明来源，不能证明授权；
- 初始 Work 决策若选择与用户目标无关的 action，必须拒绝并重试或阻断；
- 完成评估必须绑定原始目标的 required capabilities/evidence；文件请求没有 `file.read/search/list` 回执不得完成；
- `Work 已验证` 只能来自定义好的兼容性 eval，不得由任意一次 completed Task 触发；
- 把基础设施健康、Provider 可达、协议兼容、工具成功、目标完成、发布已安装拆成独立状态；
- 从当前干净提交构建正式 profile；
- 验证 Bundle ID、版本、签名、Designated Requirement、resource seal；
- 安装到正式路径并验证 installed/build executable SHA-256 一致；
- 启动后由 UI/诊断自报同一提交和 profile；
- build/install 前后保护正式凭据、外部文件、Artifact、必要 Memory/LifeModel 数据；
- CI/本地脚本有一个正式发布完整性入口，不能只验证 target 中的 bundle。

退出条件：误路由及同类变体全部 fail closed、没有无关 Memory、任务不显示完成；正式应用截图、诊断、签名和哈希构成同一份可复核证据。

## Slice 1 — 第一条真实纵向旅程

正式应用 -> 新建 Provider Profile -> 发现/选择模型 -> 打开本地文件夹 -> 自动进入 Work -> 枚举/搜索/读取多个文件 -> 创建 Markdown 报告 -> 预览路径/来源/验证 -> 重启恢复。

实现范围：

- 真正的 provider/model profile store 与 ViewModel；
- folder Project 创建、选择和恢复；
- `folder.list`、`file.search`、`file.read` 的受限契约；
- Markdown Artifact 写入 Project 的安全路径；
- preview、source/evidence、result；
- 重启后 Project、Conversation、Task、Result 和选择状态恢复；
- 删除被这条旅程替代的旧 name-only Project 和单配置伪 Profile 路径。

退出条件：正式原生成功、失败、取消、离线、模型不兼容、非 ASCII 路径、空目录、嵌套目录和重启变体全部通过。

## Slice 2 — 文件编辑与可逆性

- 修改/创建/重命名普通项目文件；
- 多文件 diff；
- accept/reject；
- checkpoint/undo；
- 并发漂移与 precondition 失败；
- DOCX/XLSX/PPTX/PDF 的创建与可视化验证，文本/代码/JSON/YAML/CSV/PDF/Office/图片的读取。

退出条件：真实文件哈希、diff、回滚和重启后的状态一致；没有通过 managed Artifact 冒充普通文件编辑。

## Slice 3 — 长任务、Review 与恢复

- 计划、进度、steering、stop、后台继续、resume；
- JIT Review；
- 网络/外部写入/破坏性动作边界；
- provider timeout/quota/offline/invalid structured output；
- 直接恢复动作和 retry-as-new-run；
- 通知只在有真实原生 activation owner 后启用。

退出条件：运行中、等待决定、失败、远端未知、取消、恢复和完成都能从正式 UI 到持久化事实闭环。

## Slice 4 — 历史与个人智能

- Projects/Conversations/History 的统一导航和搜索；
- rename/archive/restore/delete 的清晰生命周期；
- Agent Memory 显式记住、查看来源、编辑、忘记、撤销；
- LifeModel 仅作为用户拥有的可选增强；
- 普通任务不因 Memory 开启而获得无关写能力。

退出条件：显式 Memory 与普通文件/聊天的对照测试通过；错误提示词不能污染长期状态。

## Slice 5 — Web 与独立资源

- 公共 Web 搜索、页面读取、来源引用和不足声明；
- standalone 文件、URL、导入文档、图片和资源；
- no-Project 与 Project 两种上下文；
- 网络关闭、域名拒绝、来源冲突和引用漂移。

退出条件：正式原生来源可点击、声明受当前 Run 证据约束，来源不足不会伪造完成。

## 12 条旅程验收矩阵

每条旅程至少覆盖：

- fresh / returning；
- local / cloud / custom / unavailable model；
- no Project / selected Project / stale or missing folder；
- success / failure / cancel / restart；
- ordinary / non-ASCII / long path；
- keyboard / VoiceOver / 200% zoom / reduced motion；
- normal window / narrow window；
- no credential leakage / no external file leakage。

固定旅程：

1. 首次启动、Provider、模型；
2. 无 Project Chat；
3. 打开文件夹 Project；
4. 枚举、搜索、读取；
5. 创建、修改、重命名、预览、diff、撤销；
6. 长任务计划、进度、steering、stop/resume；
7. JIT Review；
8. 失败与恢复；
9. 历史、搜索、归档、删除；
10. Agent Memory/LifeModel；
11. Web 研究与引用；
12. 独立文件、URL、资源。

## 防漏策略

场景测试不能穷举自然语言，因此每个切片同时维护以下产品不变量：

- 目标不匹配不得完成；
- 所需能力没有成功回执不得完成；
- 未明确要求不得写 Memory/LifeModel；
- 未观察文件/来源不得声称读取；
- 未物化不得声称文件已创建；
- 未通过正式包不得声称已交付；
- stale/unknown/remote-unknown 保持阻断或未知；
- 任何 scope expansion、敏感披露、破坏性/不可逆动作和 LifeModel 变化走对应 JIT Review。

除固定脚本外，每个 Slice 做一次从空 Profile 开始的探索式原生 QA，用真实模型自然表达目标，专门寻找场景集之外的问题。

## 数据与清理

- 开发期不做复杂历史迁移；只保留正式凭据、用户外部文件、真实 Artifact、必要 Memory/LifeModel。
- QA 历史数据在证据归档后可清理，避免旧失败和旧成功污染首次使用。
- 每条新链路验收后删除对应旧实现；删除前追踪真实消费者，删除后跑正式原生回归。
- 不创建额外 OpenLife checkout/worktree，不触碰无关用户变更。

## 当前下一动作

Gate C 已于 2026-08-24 通过最终复核并冻结首轮设计基线。12 条旅程的原型、
交互合同与复核证据分别位于 `docs/prototypes/openlife-product-experience/`、
`docs/architecture/product-experience-contract.md` 和本轮可视化审计输出。

Slice 0 的生产修复已将 Personal Intelligence 从 Work 初始
能力集中移除，在独立显式意图证明存在前拒绝 Task 内 Memory/LifeModel 写入，
并删除普通 Chat/Work 完成后的后台 Memory 抽取路径；对应的 Project 文件读取
误路由回归与全量 Rust 检查已经通过。

第二组生产修复已在初始 Work 决策前增加独立的 authenticated goal contract：它
只声明受限 capability 下限与完成 evidence，不携带路径、参数或权限；runtime
把该合同绑定进计划和完成评估。Project 文件目标现在不能被直接答案或无关 Web
计划替代，没有 required `file.read` 成功回执就不能完成。普通 completed Task 也
不再把 Profile 标成 `Work 已验证`；只有未来的版本化兼容性 eval 可以产生该状态，
已观察到的结构化协议失败仍保留精确负面状态。

第三组生产修复已经加入版本化、固定响应的 Provider/model Work 兼容性 eval；
可达性、Chat、Work 与工具状态由 backend Profile ViewModel 分开投影，普通 Task
完成不再产生兼容性信用。正式发布入口现在从 release 构建、签名、Designated
Requirement 与 resource seal 一直验证到显式安装路径和 executable SHA-256；正式
profile 对 dirty checkout 保持 fail-closed。QA profile 已完成真实签名、安装、哈希
一致和 UI 自报验证，并明确显示当前包包含未提交源码；正式 profile 未被替换。

原生 QA 还确认了一条历史 Project 文件任务曾被旧 Work 个人智能路径错误写成
Memory 并标为完成。新 runtime 回归已阻止同类误路由；read model 现在也撤销所有
历史 `work_memory_*` FinalResult 的交付信用，保留审计记录但投影为缺少完成证据，
避免旧错误成功继续污染产品体验。

Slice 1 的首段已经贯通：原生“打开 Project 文件夹”会保留尚未落库的新对话草稿、
自动进入 Work，不能被该 Project 的旧会话覆盖；`folder.list`、`file.search` 与
`file.read` 都绑定 Project revision/scope，返回相对路径且不跟随 symlink。受控端到端
测试已从目录发现、文件搜索和精确读取继续到 Project 内 Markdown Artifact 的直接
物化与验证，只有成功 `file.read` 和已验证 Artifact 同时存在时 Task 才能完成。

Slice 1 的结果与首轮恢复也已接入：正式结果 UI 默认只展示文件位置、预览和常用
动作，来源、摘要核验与撤销收进一个按需展开的详情；v9 ConversationStore 会原子
保存当前会话，显式切换先持久化再加载，归档时清除失效选择。QA 原生数据库已从
v8 升级到 v9，102 条既有 Conversation 全部保留；选择 Artifact 会话后重启仍恢复
同一 Project、Conversation、Task 和 Result。

Slice 1 的原生失败矩阵已补齐到当前可执行边界：取消运行持久化为 `cancelled` 且没有
FinalResult；不兼容的本地模型重复工具动作时以 `agent_step_tool_call_duplicate` 阻断；
空目录只有成功 `folder.list`，没有 `file.read`、FinalResult 或新文件，并以
`work_run_budget_exhausted` fail closed；隔离离线进程使用同一已签名 QA executable 和
未监听的 loopback Ollama 地址，界面明确显示模型不可用、禁用发送，且没有创建 Task。
正式 Slice 0 安装门仍待工作区形成干净、可审计提交后执行；该门不会被 QA 证据替代。

真实 Provider dogfood 已补充两项事实：新草稿先选 `ollama/llama3.1:latest` 再选择
Project 时，Profile/model/reasoning 保持不变，不再回退到旧 Conversation 的模型；
同一受控 Project 文件任务已经在签名 QA 原生包中完成 `read_workspace_file -> verify ->
deliver_result`，持久化 `file.read` 调用、文件观察、独立语义核验和 FinalResult，并在
对话中交付一句回答。Project 目录仍只有原文件，没有网络或写入工具调用。

这次 dogfood 同时清理了四类静态 Ollama 契约漂移：初始决策现在投影当前 runtime
函数；无 MCP 目标的计划不再携带空 `targetId`；终态 Answer/Artifact 使用来源感知的
判别联合；语义核验 schema 绑定当前 Run 的 requirement ID、candidate ref 与真实
evidence ref。一次普通成功只能清除更旧的 observed contract failure 并回到
`unverified`，仍不能冒充版本化 `Work 已验证`。任务重试成功后会同步重读 Conversation，
不再同时显示旧的“本轮失败”或“这不是完成证明”。

非 ASCII 与嵌套目录的真实原生链路也已通过。受控文件
`嵌套 目录/更深一层/用户 标记.txt` 由运行时从用户明确引用且 Project 内真实存在的
路径绑定为 provider-native 枚举；有精确路径时不再向模型暴露无必要的目录发现动作。
普通最终答案的 evidence/artifact ID 由 runtime 绑定本轮真实收据；只有单一证据源时
才可无歧义修复结构化 claim 的 source ref，本地工具结果不会伪装成 Web/导入文档来源。
签名 QA Task `ebdebeae-08d9-49ae-83f4-66d165cb3493` 完成唯一 `file.read`、观察、独立
语义核验和 FinalResult，逐字交付“非 ASCII 与嵌套目录的真实读取标记：青鸟 2026。”；
文件 SHA-256 仍为 `82a2360907d43611ee2042d17427af0fce1ba17b9b598dc54ee207baed7341e9`，
Project 仍只有原来的两个文件。当前签名 QA executable SHA-256 为
`d1ee0e24ef0ae3adddc8ae4bdce756953c214c126879cdf864fb245c91f56cb7`，built/installed
hash、Bundle ID `ai.openlife.desktop.qa`、签名与 resource seal 均已再次核对一致。

Slice 2 已开始沿现有 canonical Artifact spine 纵向收口。替换普通文本文件时，Review
read model 现在从 runtime-owned draft 与 pre-change snapshot 校验摘要后生成受限的
unified diff；创建文件则以“当前不存在”作为基线。这个投影只改善可读性，不改变
Proposal identity、precondition、accept/reject eligibility 或 effect contract；diff
截断时仍明确提示批准绑定完整内容摘要。现有替换测试已覆盖“原文件保持不变 -> Review
显示精确删除/新增行 -> accept 写入 -> undo 恢复原内容”。Artifact 目标模式现已成为
authenticated goal contract 的显式枚举：`none / new_file / replace_existing / rename_existing`。替换现有文件
可绑定用户明确引用且已鉴权的 1-5 个 Project 文件；每个目标都必须在本轮由成功的
`file.read` 观察，多个草稿按不区分大小写的精确 basename 一一绑定。缺少目标、模型发明文件名、
重复 basename 或未读取目标都会 fail closed；单文件替换仍忽略 Provider 的 `suggestedName`，
额外只读目录也不能获得写权限。Ollama 的 structured-output schema 已同步该合同，避免 schema
与 Rust 类型漂移。

真实签名 QA Task `c07c540e-af77-4dde-83f7-4ef28e32c6a3` 的首个 Run
`920e162c-0882-4b64-b869-e039c6197957` 暴露并定位了上述 Ollama schema 漂移；同一 Task 的
新 Run `465f2c5d-cf13-4710-8994-7cec64b80b9f` 随后读取并修改真实
`旅行计划.md`，Review 精确显示“自由活动 -> 平江路”和“1200 -> 1500”，批准后目标摘要为
`7b148af314f76bec8b89bbb8c1246b064a16a1b41df1f75daf1f63b2d544a8af`。审批响应的严格 IPC
类型已补上 canonical projection 状态，避免真实写入成功后界面误报“决定记录失败”；重试成功
也会清除该 Task 所有旧 Run 的未决 attention。既有 QA 历史记录由旧包完成，按开发期不做复杂
迁移的约束保留原审计事实，新回归只保证后续完成不再产生这种陈旧提示。

同一原生旅程的受治理 Undo proposal `bc9376e3-35f2-4c2a-8448-673f131b4e43` 已作为当前
Work 决定节点展示，标题为“确认撤销文件修改”，批准前显示精确反向 diff；批准后 proposal 为
`accepted / confirmed`、Undo 为 `undone`，目标文件恢复到原始摘要
`a26e6d09af4c5c6434baad80fbbbd628585eacff41640c7ea6d3475d7be3421e`。结果卡现在明确显示
“已撤销 / 撤销已核验”，保留撤销前历史预览但不提供打开入口，不再把主动恢复误报为内容漂移。

多文件纵向链已在签名 QA 原生包中完成真实 dogfood。Task
`fcacf0b8-f778-48ca-a3df-880b090d5cf7` 对 `README.md` 与 `notes.txt` 产生两个 Provider-native
`file.read`、两个 observation、两个 Artifact draft 和两个独立 Review checkpoint；审批前文件摘要
分别保持 `b1056803f2e5fddeedd63f44e0c6bc5f9d11edf4101e8d0c1bc2b8f2edf5989a` 与
`dc4b120aa1e09b58b213e4870683c61def90c642539fb520d6300df2611e6706`。内嵌 Review 只增加紧凑的
“2 项修改，逐项确认”切换器，每个文件显示自己的精确 diff。第一项批准后只有该文件写入，另一项
保持原摘要且 Task 仍为 `waiting_review`、没有 FinalResult；第二项批准后 Task 才成为
`completed / delivered`，最终摘要分别为
`d8f4abfd6c6b1685a035a0d3c0f7d854b54ea195adf246214535662fef096659` 与
`d82a65c262e2177aa61d13ef9e40d847cf28abda97d1fb9571d5d5d4f0f1b57a`，两项都保留 Undo。

这次真实 Provider 运行先后暴露两条契约缺口：Ollama 的 goal capability 已声明
`draft_artifact`，但冗余 `completion.resultKind` 返回 `answer`；运行时现在只在这一种严格错误下，
机械地由已声明 capability floor 归一化 result kind，不推断目标、权限、路径或语义。其后模型只读
第一个文件就尝试起草，暴露多文件 read floor 被任意一个成功读取错误满足；调度器现在持续收窄为
尚未读取的鉴权目标，所有目标都有成功收据前禁止终态。Core 数据库重启测试另行证明两个 checkpoint
跨 reopen 保留，批准一个不能完成 Task，批准全部才交付。现有 CAS 漂移门仍逐文件绑定审批时基线；
当前语义是逐文件审批与物化，不宣称不可分割的批量原子提交。

最后一次原生自查还发现：最终审批后，旧阻断历史会替换刚完成 Task 的详情；审批动作现在先绑定其
所属 Task，默认选择也不再把终态 blocked 当作仍在执行。重启后的签名 QA UI 已验证最新成功 Task
保持选中，旧失败仍留在活动历史，没有运行中任务时顶部如实显示“没有当前任务”。

Project 文件重命名的纵向链现已完成。`rename_existing` 只接受用户在当前消息中明确引用的一个已存在
源路径和一个尚不存在的目标路径；源文件必须由本轮成功 `file.read` 鉴权，源/目标必须位于同一
Project 目录并具有与 Artifact kind 一致的扩展名。Provider 的建议文件名和重写内容均不能获得
权限，运行时用已读取的精确字节绑定 move；目标已存在、路径含 traversal、来源未读取、目标未被
用户明确引用或审批后源摘要漂移都会在副作用前 fail closed。move 始终进入 JIT Review，结果卡投影
“重命名文件”，并通过 schema v27 的 `restore_moved` Undo 记录支持重启后的反向 move。

真实签名 QA Task `d724744f-321e-4e78-8ec3-e330029441f4` 使用本地
`ollama/llama3.1:latest` 读取 `rename-source.md` 后进入“确认重命名文件”；批准前源文件仍存在、
`rename-target.md` 不存在，批准后只有目标存在且 SHA-256 保持
`c712f5198e7756f43cba4c3748f6e576f90e0776b9c201d0c3fe1def702902d3`。首次真实运行暴露 Goal
提示把 `draft_artifact` 错误限制为“独立文件”而与 rename 规则冲突；现在明确任何持久文件效果都要
声明该 capability，并只按模型已声明的非 `none` 目标模式机械修复冗余 artifact/result 字段，不用
关键词推断语义。首次 Undo 又暴露 Review Center 未把 `restore` 识别为完整 move precondition；
修复后 proposal `74e24f5a-7f35-4c09-961e-841f638e6037` 在应用重启后仍可批准，原名称与同一摘要
恢复，目标消失，结果卡明确显示已撤销。

多文件结果现在只在同一 Task 至少有两项可撤销 Artifact 时显示一个紧凑的“撤销全部修改”入口；
后端仍从 canonical Task 重新枚举 Materialized 且没有既有 Undo 的 Artifact，逐项生成受治理 proposal，
并以 `waiting_review / partial_waiting_review` 收据明确区分全部与部分创建成功，不把多文件恢复冒充原子
事务。真实 Task `fcacf0b8-f778-48ca-a3df-880b090d5cf7` 已通过该入口生成两个决定节点；第一项批准后
只有 `notes.txt` 恢复到 `dc4b120aa1e09b58b213e4870683c61def90c642539fb520d6300df2611e6706`，
`README.md` 仍保持修改后摘要，第二项批准后 `README.md` 才恢复到
`b1056803f2e5fddeedd63f44e0c6bc5f9d11edf4101e8d0c1bc2b8f2edf5989a`。结果页随后投影
“2 个产物已撤销”。替换内容的 Undo Review 按真实操作改为“批准并恢复文件”，不再误写为普通写入。

当时全量 Rust 671 + 420 + 2 binary + 2 doctest、Clippy、fmt、diff，以及前端 280 tests、
typecheck、format 和 production build 均通过。当前签名 QA executable SHA-256 为
`4c6df5596341f0ec5970c7401eb844363419efb7740256dc0d948b61b2cb7be9`；built/installed hash、
Bundle ID `ai.openlife.desktop.qa`、签名与 resource seal 一致。工作区仍为 dirty，因此正式
`/Users/tw/Applications/OpenLife.app` 未触碰，QA 证据不冒充正式发布证据。

多文件 Undo 的部分失败与 restart reconciliation 已收口。批量入口会在创建决定节点前重新核验
每个当前文件；若某项在 OpenLife 完成后被用户或其他工具改写，该项以
`artifact_undo_source_changed` 保留现状，其他可恢复项仍各自进入 Review，前端刷新后明确播报
“未覆盖这些新内容”。ProposalStore 与 CanonicalTaskRuntimeStore 之间的提交窗口也已建立回归：
系统生成的 active Undo proposal 若已持久化但 canonical Undo checkpoint 缺失，启动 reconciliation
会校验幂等键、Task/Run/Artifact/版本、原 proposal 和逐操作路径/摘要后只重建 checkpoint，不执行
文件副作用。文件型 SQLite reopen 测试证明重建后两个决定节点仍能逐项批准并恢复各自原始摘要。

Office 创建与可视化验证的第一轮已经收口。DOCX/XLSX/PPTX 仍由确定性 OOXML adapter 生成并立即
经产品 parser 重读；现在显式声明中日韩文字字体，DOCX 有正文行距与标题层级，XLSX 有自适应列宽、
表头、冻结首行和筛选，PPTX 有明确白色背景、色彩与视觉锚点。macOS Quick Look 已逐项显示中文与
版式，外部 LibreOffice 引擎已逐项成功加载；canonical Work 回归进一步覆盖中文文件名、中文内容、
三个独立 Review、写入、摘要核验和物化后重读。结果页对 PDF/Office 只标记“提取内容”，不再把
可搜索文本冒充文件视觉预览。

PDF 创建现已通过第二条实现收口，第一条被否决的非嵌入字体方案仍保持删除。新 adapter 使用仓库内
固定的 `NotoSansCJKsc-Regular.otf`，记录上游 commit、SHA-256 与 OFL，并由 `printpdf` 在保存时嵌入
子集字体；遇到字体缺失字符（例如 emoji）会以 `artifact_pdf_font_missing_glyph` fail closed，不静默
丢字。代表性 3 页、72 段中文报告为 A4、89,570 bytes，产品 parser 和外部 `pdftotext` 均能读到末项；
Poppler `pdffonts` 独立报告 CID 字体的 embedded/subset/Unicode mapping 全部为 yes，三页 PNG 人工检查
无乱码、裁切或空白。PDF 也已进入 Agent format、扩展名/MIME/base64 边界以及 canonical Work 主链；
同一 Task 的 DOCX/XLSX/PPTX/PDF 四份中文产物分别进入 Review，批准后完成摘要核验、物化与格式解析
重读。

真实模型和签名 QA 的 PDF 纵向旅程也已完成。首次使用本地 `ollama/llama3.1:latest` dogfood 先后暴露
goal contract 错把 source-independent 新文件当 source evidence、直接 Artifact 所需 `verify` 未进入
初始决策能力下限，以及多格式 Artifact function schema 允许 PDF content 退化为字符串。前两项已按
真实合同修复；多格式 schema 已改为 format-discriminated typed contract，并在任何文件 Artifact 上统一
走 `plan -> draft -> independent verify -> review -> materialize`，不再要求较弱模型在初始回合同时选择
plan 与深层多格式 Artifact 工具。系统仍对错误 PDF content fail closed，不用关键词路由或静默包装来
制造成功。

随后按用户选择切换到已配置的 `openrouter/stealth/ox-alpha`，在同一真实 Project 文件夹完成 Task
`9f2f14a1-2db2-432d-a591-45b3cb2285b3`、Run `fc85ebb1-c60b-45e3-a9cf-95b5b0dfada7`。Review 前
`原生PDF核验.pdf` 在磁盘上不存在，界面显示精确标题、两个小节、目标路径与“批准并写入文件”；批准后
Artifact `artifact:4d5462aed9d867a7228d4c733e453ebfd50fa0c2670aa44233a0e3c8e837c1a9`
物化到 Project，SHA-256 为 `6e928d8cd0a79a275ee0ea5956a727ab69032b8d48e6a4b8bcda591bd8e6e59b`。
外部 `pdfinfo` 报告 A4、1 页、74,889 bytes，`pdffonts` 报告 Noto CJK CID 字体 embedded/subset/Unicode
均为 yes，`pdftotext` 精确读回标题、两节正文；Poppler PNG 与 macOS Preview 都显示中文无乱码、裁切或
空白。应用重启后仍恢复同一 completed Task、物化路径、提取内容和打开动作。当前签名 QA executable
SHA-256 为 `e6b0f1a4db82805ae60616e8f33a19a16fd71b8a36c4ec06eba08b186a90862c`，built/installed hash、
Bundle ID `ai.openlife.desktop.qa`、签名与 resource seal 一致；正式应用仍未触碰。

并发漂移的第一条端到端门也已补齐：Review 打开后若用户或其他程序改写目标文件，
commit CAS 会返回 `artifact_target_precondition_changed`，保留用户的新内容、没有
FinalResult，并把 Artifact/Task 确定性标为 failed-before-effect。此前该 commit 分支会把
所有 filesystem failure 一律升级为 `effect_unknown`；现在只对真正无法确认副作用的
失败保留 unknown，已证明未写入的 precondition failure 不再制造虚假不确定性。

Project 二进制文档读取已沿既有 `file.read -> ToolGateway` 主链补齐，不另建资源平台，也不把
Project 文件静默导入为绑定 Resource。文本文件仍保持 100KB 的 UTF-8 读取合同；PDF、DOCX、
XLSX、PPTX 则允许最多 20MB，在通过 Project root、canonical path 与 ToolGateway 授权后交给
现有 killable `ResourceParserProcess`，返回有界的 `project_document_extraction` 观察结果。观察
保留 PDF 页码、DOCX 段落、XLSX 工作表/单元格范围和 PPTX 幻灯片 provenance，最多 64K 字符、
64 个 chunk，明确标记截断；parser 不可用、MIME/magic 不一致、损坏或超限均在副作用前 fail
closed。图片仍未冒充已支持：当前 Provider `ChatMessage` 只有字符串，没有经过 profile 能力校验
的多模态 content parts，下一步必须先建立 provider/model 图像输入合同。

真实签名 QA 包已重新构建安装到 `/Users/tw/Applications/OpenLife QA.app`，Bundle ID
`ai.openlife.desktop.qa`，executable SHA-256 为
`a38b8fffadfee1cfabb028fe30c63d02e0806baf18728f9f2a6845a3a1b9acf1`，built/installed hash、
本地签名与 resource seal 一致，上一包备份为
`/Users/tw/Applications/OpenLife QA.app.backup.20260825T085140Z`；dirty checkout 下正式应用仍未
触碰。`openrouter/stealth/ox-alpha` 的首次三文件 Run 在 provider 计划阶段明确失败为
`provider rate limited`，没有伪造读取；本地 `llama3.1:latest` fallback 只完成目录枚举后长时间
未选择 `file.read`，取消后保留真实 `cancelled` 证据。将 ox-alpha 推理强度明确设为 low 后，Task
`6a26d48e-4186-44c5-a242-d794571ad05b` / Run
`ae842039-36da-4e5a-b69f-b4ecaab2fb9b` 真实执行 `folder.list -> file.read`，从
`combined-report.pdf` 第 1 页读出 `COMBINED_REPORT_PAGE_ONE` 及同页正文；Task
`77e07e7a-a5a7-40a1-9e35-c84e2458441a` / Run
`f3165db0-a648-435e-946a-000e0f67ca33` 又分别读取 `checklist.docx` 与 `metrics.xlsx`，交付
`ROADSHOW_CHECKLIST_SENTINEL`、`RESOURCE_ROW_SENTINEL` 以及精确 provenance
`roadshow_metrics / A3:D3`。三项文件读取均有 canonical tool attempt 和最终交付，不把 parser
单元测试、worker fixture 或模型开始响应当作原生完成证据。当前新增的三项 observation 单元测试、
三项真实 parser-worker binary 测试、原有 parser 回归、Clippy、typecheck 与前端 281 tests 均通过。
应用退出并重新启动后，同一 Conversation 的两项 completed Task、推理 low、完整工具时间线、PDF/
DOCX/XLSX 最终回答与精确 XLSX provenance 都从 canonical store 恢复，未退化为临时 UI 状态。

Provider/model 图像输入的代码合同现已沿现有 Project `file.read -> ToolGateway -> canonical Work ->
ProviderClient -> adapter` 主链补齐，没有另建上传平台。OpenRouter 的官方模型发现结果会按精确 model id
提取并验证 `text/image/file/audio/video` 输入模态；未取得官方发现结果时 profile 仍保守为 text-only。
Project 图片只接受 PNG/JPEG/WebP/GIF、单项最多 20MB、每次最多 4 项；tool observation 只持久化相对路径、
MIME、字节数和 SHA-256，不持久化 base64。模型调用前运行时重新解析受治理 Project 路径、重读字节并核验
magic、大小与摘要，文件漂移或 profile 未声明 image 能力都会在 Provider dispatch 前 fail closed。OpenAI/
OpenRouter 和 Ollama adapter 都已有真实 multipart message content 传输，序列化测试证明瞬态图片字节不能随
请求对象落盘，本地 HTTP adapter 测试也精确核验了最终 `data:image/...;base64` 请求体。

当前 Rust core/desktop 全量测试（含真实 parser worker）、Clippy、fmt、diff，以及前端 typecheck、281 tests、
format 和 production build 全部通过。新签名 QA 安装包位于 `/Users/tw/Applications/OpenLife QA.app`，
Bundle ID `ai.openlife.desktop.qa`，executable SHA-256 为
`ffac760442734a92b7bc483a5f6427c79d64a05c9731d3b2a39789b1c836bc51`，built/staged/installed hash、
签名与 resource seal 一致；dirty checkout 下正式应用仍未触碰。

真实远端图片旅程保留为部分证据而不冒充完成。Task `1ab38838-1e85-45d0-94c7-964821c6fa8c` 使用
`openrouter/stealth/ox-alpha`、reasoning low 成功执行图片 `file.read`，第一轮 Run
`a6881914-bf44-480c-a66c-d00570786ddc` 的图片绑定后 Provider 生成也已完成，但最终语义验证命中
`provider_rate_limited`；第二轮 Run `328b85b7-5005-47a5-81c0-c1566f9e5e66` 在图片绑定后的首次
Provider 调用命中同一外部限流。应用如实显示失败和重试，没有制造 FinalResult；QA 数据目录扫描未发现
`data:image`、JPEG 或 PNG base64 标记。另行探测 `google/gemini-2.5-flash-lite`、
`openai/gpt-4.1-mini` 和 `google/gemma-4-31b-it:free` 均被 Provider 的精确连接测试拒绝，未保存。
因此图片代码与受治理传输合同已完成，但“原生远端图片最终交付”仍待一个不被限流的真实 Provider 窗口。
下一纵向切片继续拆除当前单一 `config.yaml` 生成伪 profile 的做法，建立小型、真正持久化且被运行时消费的
Provider/Model Profile；不会用更多设置项或 capability 徽章掩盖该缺口。

远端模型选择的第一条可执行纵向链已完成。OpenRouter 模型来自官方 `/api/v1/models` 目录，只有输入包含
text、并同时声明 `tools`、`tool_choice` 以及 `response_format` 或 `structured_outputs` 的模型才进入
OpenLife 当前 Agent 候选；目录元数据只证明模型合同兼容，不冒充账号 entitlement 或真实调用成功。工作区
模型入口已收敛为一线 Agent 常见的紧凑按钮与搜索弹层：空查询只展示当前模型和本地模型，远端目录在用户
搜索后出现，不再把 Chat/Work/协议诊断常驻在主界面。搜索 `gpt-4.1-mini`、选择
`openai/gpt-4.1-mini` 后，真实 Turn `4e4f03f1-9022-4c7b-8119-83596ae21fba` 已在 canonical store
记录精确 provider profile、model id 与 cloud endpoint，证明选择不再只是前端标签；该次外部调用失败，
因此没有宣称模型回复成功。

这次原生 dogfood 同时暴露并修复了两个真实运行时缺陷。其一，从 Settings 返回 Workspace 只刷新聚合
Workbench，未刷新 Conversation ViewModel，导致刚通过的 `openrouter/stealth/ox-alpha` 精确连接验证仍显示
不可用；现在返回时同步 reload Conversation，并有回归覆盖。其二，Provider 已开始生成后失败时，Chat runtime
会直接返回错误而把 Turn 永久留在 `running`；现在 Provider failure、非法 AgentStep、不可执行 personal action
和禁用 step 都会先写入确定性的 failed 终态。新的真实失败 HTTP adapter 回归证明 dispatch 后失败也会终态化。
模型切换所派生的 runtime generation 也由随机 UUID 改为基于父 generation、route 和 model 的确定性摘要，
既保持运行时身份隔离，又不破坏同一请求的幂等 replay。Rust core/desktop、Clippy、fmt、diff，以及前端
typecheck、281 tests、format 和 production build 已全部通过。这里仍不宣称完整 Provider/Model Profile 系统：
当前远端目录 profile 是由一个已配置连接在运行时派生，持久化多 Provider 连接与用户模型偏好仍是下一切片。

Provider/Model Profile 已从纯 ViewModel 列表向真实持久化实体推进第一段。ConversationStore schema v10
新增 `provider_connections` 与 `provider_model_profiles`：Connection 保存 provider、endpoint、非秘密
credential reference/version、协议、隐私边界和验证状态；Model Profile 保存精确 model id、展示名以及有界的
provider/adapter capability snapshot。API Key 仍只在既有 secret store，不能进入数据库。当前 Conversation 和
新 Conversation 默认选择分别持久化；composer 点击模型时必须先由后端 registry 证明该项 ready，再把 Connection、
Model Profile 与选择作为 canonical ConversationStore effect 写入，失败不会只改变前端标签。Conversation ViewModel
优先读取当前 Conversation 的持久化选择，Chat/Work 发送仍用同一 profile id 解析精确 scheduler，因此该记录已经被
产品运行链消费，而非孤立设置表。

v9→v10 migration、文件型 reopen、非法/不可用选择 fail closed、后端选择与前端 IPC 回归均通过。最终签名 QA
直接打开原有 v9 `conversations.db` 后迁移为 v10，旧 Project、Conversation、Turn 与 Work 历史均保持可读；在原生
模型弹层搜索并选择 `ollama/llama3.1:latest` 后，数据库写入 Connection
`provider-connection:b176971a526781d945a0abe8`、Profile
`provider-profile:8954e7f7ed622e8634d5a389`，并绑定当前 Conversation
`d46c466f-ea80-4da3-aee3-378f85efcb35`。退出并重启应用后 composer 仍恢复“本地”模型。当前 OpenRouter
精确 `stealth/ox-alpha` 连接测试收到 `provider_confirmed_failure`；官方实时目录仍能发现该模型及 typed tool
合同，但账号/请求可用性没有通过，因此未把它写成成功 Profile，也没有发送新的聊天来制造假证据。

全量 Rust core 684 passed / 3 ignored、desktop 424 passed / 2 ignored、真实 parser binary 3 passed；前端
typecheck、281 tests、format、production build，以及 Clippy、fmt、diff 全部通过。最终 QA executable
SHA-256 为 `c8dba17d9cd9754af41234bb36605019803cf55c646ec1591bd21f6b0e868319`，built/staged/installed
hash、Bundle ID `ai.openlife.desktop.qa`、签名与 resource seal 一致，备份为
`/Users/tw/Applications/OpenLife QA.app.backup.20260825T105813Z`；dirty checkout 下正式应用仍未触碰。
这一段仍不宣称整个 Profile 生命周期完成：Settings 的多 Connection 新建/编辑/删除、为不同 Connection 绑定独立
credential reference，以及存量 `config.yaml` 单连接写路径的最终删除，继续作为下一纵向切片。

Provider/Model Profile 生命周期的第二段已完成。ConversationStore schema v11 为每个精确 Model Profile 保存独立
验证状态，避免测试同一 Connection 下的一个模型后错误地把其他模型标为可用；Settings 现在提供紧凑的 Connection
列表和单个行内编辑器，支持新建、编辑、测试和删除，不展示 capability 矩阵或额外平台界面。每个 Connection 使用独立
credential reference，数据库仍不保存密钥；测试只更新被测试模型的状态，删除 Connection 会级联清理 Model Profile 与
失效选择。运行时 registry 会从持久化 Connection/Profile 构造精确 endpoint、credential、model scheduler，选择记录继续
作为 canonical ConversationStore effect 被 Chat/Work 共用。

存量 `config.yaml` 云端连接会在启动时诚实导入为未验证 Profile，即使数据库已经有本地 Profile 也不会漏迁移。签名 QA
原生应用导入了 `openrouter/stealth/ox-alpha`；首次编辑在不重新输入 API Key 的情况下，把旧全局 secret reference 复制为
该 Connection 独立 reference，同时保留旧全局凭据，重启后仍显示同一 OpenRouter、精确模型、endpoint 和“待测试”状态。
原生界面没有把迁移或模型目录元数据冒充为连接成功，也没有再次触发受限的远端调用。QA executable SHA-256 为
`34e5d0676f763186d7d858b3f89b65b689bb665d6fc32ce1987a4b9fa6cff07d`，built/staged/installed hash、Bundle ID
`ai.openlife.desktop.qa`、签名与 resource seal 一致；dirty checkout 下正式应用仍未触碰。

最终全量回归为 Rust core 688 tests、desktop 426 passed / 2 ignored、真实 parser binary 3 passed、doc tests 2 passed；
前端 31 files / 282 tests、typecheck、format、production build，以及 Clippy、Rust fmt、diff 全部通过。下一纵向切片不再
增加设置项：在已完成启动迁移和独立凭据路径的基础上，拆除 `config.yaml` 作为云端 Provider 的运行时/写入权威，让持久化
Connection/Profile 成为唯一产品 owner；旧配置只保留一次性迁移读取，随后再做一个真实远端成功窗口的原生交付验收。

旧单连接权威的第一段拆除已经继续完成。正式与 QA 构建的模型 registry 不再把 `config.yaml` 云端字段生成动态 Profile，
也不再从该单连接生成 OpenRouter 全目录的“可用”模型；云端模型必须来自 ConversationStore 中真实存在的 Connection 与
精确 Model Profile。云端 Profile 若缺少持久化 Connection，执行解析会在网络调用前以
`provider_profile_connection_missing` 失败关闭。受控单元测试仍保留显式 harness seam，但产品构建不能进入这条路径。
本地 Ollama 发现与持久化本地选择不受影响。

通用 `save_config` 也不再接收前端提交的 `llm` 字段作为 Provider、endpoint、模型、credential reference 或版本的写入
权威，并且保存本地模型、网页搜索等无关偏好时只轮换独立 Search 凭据，不再顺带重写旧的全局 Provider secret。
首个测试成功且工作区尚无默认模型时，精确 Profile 会成为默认选择；已有本地或其他用户选择时不会被覆盖。新增回归证明
产品 registry 不暴露云端旧配置、通用设置注入不能改变 Provider 权威、Search 保存不接触 Provider secret。

新的签名 QA 安装包已在存量 schema v11 数据上启动。模型选择器只展示当前/已验证的持久化模型与真实本地发现结果，
未验证的 `openrouter/stealth/ox-alpha` 仍保留在 Settings 的独立 Connection 中并显示“待测试”，旧配置没有再次制造
重复或伪 ready 模型。QA executable SHA-256 为
`c932b21ba21e0de2da32f0f7aa922882b8ef8a7a06c99862dfabe982b44d8967`，built/staged/installed hash、Bundle ID、
签名与 resource seal 一致，备份为 `/Users/tw/Applications/OpenLife QA.app.backup.20260825T152809Z`。最终回归为
Rust core 688 tests、desktop 429 passed / 2 ignored、parser 3、doc tests 2；Clippy、Rust fmt 与 diff 全部通过。

下一段继续处理仍依赖 `AppConfig` 的派生读模型：自动网页搜索复用应读取当前持久化 Connection/Profile 的精确 route 与
credential，而不是旧云端字段；启动凭据状态也应按 Connection 聚合。完成后，`config.yaml` 的云端字段才可以真正缩减为
一次性迁移输入，而非任何产品状态或能力判断的来源。

自动网页搜索的旧 Provider 复用路径也已拆除。Canonical Work 在创建 ToolGateway 资源快照时会传入本轮精确
`provider_profile_id`；只有该持久化 Profile 能解析为 ready Connection，且 endpoint 是 DeepSeek 或 OpenRouter 的
官方 HTTPS origin 时，`auto`/同名 hosted search 才复用该 scheduler 的精确 credential 与 model。自定义代理、另一
Provider、本地模型、缺失/未验证 Profile 或解析失败均得到 `unavailable`，不会回退到 `config.yaml` 的旧 key/model。
独立 Brave、SearXNG、DuckDuckGo 等搜索设置仍保持原有独立边界。

搜索路由回归覆盖 DeepSeek/OpenRouter 官方 origin 的精确绑定、自定义 gateway 拒绝以及不同配置 generation 的隔离；
再次运行全量 Rust 后仍为 core 688 tests、desktop 429 passed / 2 ignored、parser 3、doc tests 2，Clippy、fmt 与 diff
通过。包含该修复的最新签名 QA executable SHA-256 为
`629a96d27fac2e559536c59b80978ee5a9d0805f92c445783bc853b45b1cd3a6`，built/staged/installed hash、Bundle ID、
签名与 resource seal 一致，备份为 `/Users/tw/Applications/OpenLife QA.app.backup.20260825T154357Z`，应用已原生启动。
接下来只剩启动/设置中的 Provider 凭据状态聚合仍读取旧全局 slot，需要改为按持久化 Connection 聚合后再移除旧状态文案。

启动与设置的 Provider 凭据状态现已完成 Connection 化。`credential_bootstrap_v2` 不再读取或投影退役的
`provider_api_key` 全局槽；启动只水合仍由 `AppConfig` 独立拥有的 Search 凭据，Provider 状态则对
ConversationStore 中全部云端 Connection 的精确 credential reference、endpoint 与 version 做保守聚合。本地 Connection
不进入凭据状态；托管搜索复用当前模型 route 时也不再重复制造第二个 Provider 凭据状态。恢复确认除聚合状态外还绑定一个
不含密钥的 Connection 集合摘要；确认前 Connection 所有权、reference、endpoint 或 generation 变化都会保持 unknown，不能
把后来新增的凭据纳入旧确认。前端目的标识和文案同步改为 `provider_connections` 与“项”，旧 `provider_api_key`、v1 快照及
启动期全局 Provider hydration 已从生产源码移除。

旧单体连接测试入口现已完整删除。前端 Settings 控制器不再接受整份 `AppConfig.llm` 草稿执行测试，Tauri release handler
也不再暴露 `test_llm_connection`；唯一产品路径是已持久化的 Connection + Model Profile ID，经对应 credential reference
水合后执行精确连接测试。旧的 masked-key 回填、credential version 猜测、测试确认状态机及兼容夹具同时移除，Review 仍只
服务于当前 Connection 的精确网络授权。

设置与启动中的托管搜索判断也已切到 Connection/Profile 真相。Settings 只有在当前持久化 Profile 被选中、验证为 ready、
Connection 凭据可用且 endpoint 是 DeepSeek/OpenRouter 官方 HTTPS origin 时才显示或允许复用；通用 `save_config` 的保存后
核对完全忽略由后端拥有的 `llm` 字段，旧 Provider 字段变化不能再把无关设置保存误判为失败。启动 credential bootstrap
使用 ConversationStore 的默认 Profile 找到精确 Connection；`auto` 不再从 `config.yaml` 旧云端字段制造第二份搜索凭据
需求，显式 hosted search 只有匹配所选官方 Connection 时才免除独立凭据。`AppConfig` 中旧的 hosted-search 推导 API 已删除。
本轮回归为 Rust core 687 tests、desktop 431 passed / 2 ignored、parser 3、doc tests 2，前端 270 tests；Clippy、Rust fmt、
TypeScript typecheck、production build、Prettier 与 diff check 全部通过。

Provider 与隐私产品读模型的剩余旧权威也已拆除。Provider Privacy summary 不再读取全局
`provider_validation.json` 或 `config.yaml` 的 Provider/model 标签，而是从当前持久化 Model Profile 找到精确
Connection、按该 Connection 的 validation 文件和凭据引用计算网络与隐私边界；没有选择时保持 unselected/unknown。
产品 Readiness 同样以 registry 中选中的持久化 Profile 是否 ready 为准，不能再由旧全局连接测试把 Chat 误判为可用。
正式构建的 registry 不读取全局 validation 文件；该文件仅保留为受控测试 harness 输入。前端 Settings 的 Provider 状态
也只来自 Connection ViewModel，AppConfig 草稿仅继续承载与 Provider 无关的设置。

`config.yaml` 的旧 Provider route 现在真正成为一次性迁移输入。启动会先幂等地落盘或核对对应 Connection/Profile，复用
既有用户记录并保留已有 validation；只有持久化成功且有可保留的非明文 credential reference 后，才把 Provider、endpoint、
chat model、全局 reference 和 generation 从配置运行权威中退休，同时保留独立 embedding 偏好。仅有旧明文而无法证明安全
凭据迁移时明确失败关闭，不删除凭据、不创建伪 Connection。旧 `effective_provider_label`、`effective_openai_base` 和
`effective_openai_key` 配置包装器也因不再有生产消费者而删除。最终全量 Rust 回归为 core 682 passed / 3 ignored、desktop
434 passed / 2 ignored、parser 3、doc tests 2；前端 31 files / 270 tests、TypeScript typecheck、production build、
Prettier、Clippy、Rust fmt 与 diff check 全部通过。

前端 AppConfig 合同中的旧 `llm` 对象也已删除，不再把已退役的 Provider owner 作为 Settings 加载、编辑、保存或保存后核对
的必要字段。Provider 类型、Connection ViewModel 与保存输入使用独立 `CloudProviderId` 合同；Provider 测试 fixture 也维护
独立 Connection 状态，不再通过篡改 AppConfig 模拟 Provider 生命周期。`save_config` 的日志脱敏回归改为验证仍由该命令真实
拥有的独立 Search credential。生产前端源码现已没有 `.llm`/`llm:` 消费，后端 Settings IPC 也不再返回这些迁移字段。
前端 typecheck、31 files / 270 tests、production build、production authority guard、Prettier 与 diff check 通过。

Tauri Settings IPC 也已收紧为 `EditableSettingsConfig`，只传递本地模型偏好与 System 设置；`get_config` 不再把内部
`AppConfig.llm`、Provider route、credential reference 或 generation 序列化给前端，`save_config` 也不再接收这些字段后
再用覆盖逻辑防注入。保存时以后端当前 AppConfig 为底，只应用这份窄 DTO，并继续由独立命令保护 Artifact 目录和额外读取
根。新增后端序列化回归证明 DTO 没有 `llm` 且不包含 Provider/credential 内容；全量 Rust 仍为 core 682 passed / 3
ignored、desktop 434 passed / 2 ignored、parser 3、doc tests 2。

当前 checkout 还完成了一次明确标记为开发证据的 QA 原生冒烟，不冒充签名发布验收。本机没有可用的 Developer/Local
codesigning identity，因此使用 `ai.openlife.desktop.qa`、ad-hoc 签名的 debug app bundle；bundle 通过严格 codesign 校验，
executable SHA-256 为 `b3de16af9ee5b257491cdaf9d4f61acedf7a9e55c19a2a5231f5c70ae77a0146`。Computer Use 从真实
Workbench 进入 Settings，模型与供应商页成功读取已保存 OpenRouter Connection、独立“测试/编辑/删除”动作与本地模型偏好；
隐私与网络页成功读取 ProviderPrivacy read model、Network policy、Tool permission、Agent Memory 与 Artifact 输出目录，未出现
AppConfig `llm` 缺失、IPC 反序列化失败或 Settings 空白。测试后仅关闭 debug QA；正式 OpenLife 应用未触碰。

默认 Work 界面继续按 Gate C 收敛。Project、对话搜索、归档和读取范围等管理操作已归入默认关闭的
“管理对话与 Project”；Memory 与执行上限归入“本轮选项”；文件、Skill 与工具仍按需展开。结果页保留每个 Task 的
模型、Project、阻断和来源身份，但完整计划与长时间线在终态内嵌视图中默认折叠，运行中或等待决定时自动展开，避免
以简洁为名丢失可核验证据。原生复核进一步发现全局 Activity 在有历史失败时会默认展开并把当前对话推到下方；现在
无论历史数量多少都只显示紧凑摘要，用户明确展开后才呈现最近 12 项，历史事实和处理入口均保留。

失败恢复已经从对话状态精确绑定到 canonical Task/Run。Conversation ViewModel 返回最新 Work Turn 对应的 Task/Run ID；
前端只在同一 Task 的 backend TaskControl 明确允许时展示 retry/resume。动作会创建新的 Turn/Run，并重新核验原 Project
revision、provider/reasoning、Skill 与首个 Run 的资源范围；命令返回后同时重读 Task 和 Conversation，不能用回调冒充
恢复或完成。Provider、模型或凭据准备失败还提供直接进入持久化 Connection/Profile 设置页的恢复动作，普通错误不会
错误显示该入口。

Provider Connection 凭据的加密绑定也已脱离旧 `AppConfig.llm` 重建。SecretStore 现在只以 Connection 自身的 provider、
endpoint 与 credential version 编码和核验密钥；registry、启动聚合、Settings 与 Provider Privacy 共用这份窄合同。
endpoint 或 version 漂移会拒绝水合，旧全局 Provider 配置不能再作为凭据访问的隐式权威。正式构建的 registry 同时彻底
停止读取机器全局 `provider_validation.json`，退役的整份 AppConfig Provider/Search 测试辅助写路径已经删除。

当前全量门禁为 Rust core 682 passed / 3 ignored、desktop 430 passed / 2 ignored、真实 parser binary 3、doc tests 2；
前端 31 files / 274 tests、typecheck、production build 与 authority guard，另有 Clippy、Rust fmt 和 diff check 全部通过。
本轮另构建了未安装的 ad-hoc QA bundle，Bundle ID `ai.openlife.desktop.qa`，executable SHA-256 为
`88561c6a7414ae0183fa1af96617c818e12ad17f895a3f1762afc5e3c1a7fd9f`，strict codesign 与 Designated Requirement
核验通过。Computer Use 在真实 Tauri 窗口确认管理区默认折叠且展开后功能可达、终态计划与时间线按需展开，并从 Workbench
进入持久化 Connection/Profile 模型设置页；第二次 bundle 复核确认“全部活动 89 项需要处理”默认折叠，当前对话不再被历史列表
压到首屏下方。测试后 QA 已关闭，正式应用未触碰。该证据仍不是 Developer ID 签名或正式安装验收，
也没有执行新的真实远端调用。

Slice 2 的多文件拒绝语义现已收口。用户拒绝同一 Task/Run 中的一项文件修改时，该项保持显式
`rejected`；尚未 dispatch 的兄弟 Review 在同一个 ProposalStore 事务中转为 `cancelled`，对应
canonical Artifact、Review checkpoint、等待物化 Item 与 review-required attention 同步终态化，
不能在已经 blocked 的 Run 上继续显示可执行批准按钮。若兄弟决定已经开始 dispatch，整次拒绝以
CAS 冲突失败，不制造“已取消但副作用仍在执行”的错误事实。Task runtime schema v28 为 Artifact
checkpoint 增加 `cancelled` 并提供 v27 原位迁移；前端审核中心区分“已拒绝”和“已取消”。回归覆盖
ProposalStore 原子竞争、文件型 SQLite migration、canonical Task 多 Artifact 投影以及完整 Work
bundle 拒绝链。全量检查为 Rust core 686 passed / 3 ignored、desktop 431 passed / 2 ignored、
parser binary 3、doc tests 2；前端 31 files / 274 tests、typecheck、format、production build，
以及 Clippy、Rust fmt 和 diff check 全部通过。

Slice 2 的直接创建文件 Undo 也已补齐真实执行链。Project 内新建且目标原本不存在的文件仍可按
低风险、可恢复规则直接落盘，不额外制造初始 Review；用户随后选择 Undo 时，后端不再错误要求一份
不存在的原始写入 proposal，而是从 canonical Artifact version、Task/Run、Project revision 与真实
文件摘要证明该版本确为直接创建、原目标不存在且内容未漂移，再创建受治理的 Trash Undo proposal。
直接创建 origin 使用独立的 typed fields，启动 reconciliation 能在 proposal 已持久化而 checkpoint
尚未绑定的窗口恢复同一决定；其他无 Review 来源、摘要不一致或作用域过期的 Artifact 仍不能伪造
Undo。桌面集成回归证明文件在批准前保持存在，批准后进入 trash 并投影为 `undone`。修改后的完整
门禁仍为 Rust core 686 passed / 3 ignored、desktop 431 passed / 2 ignored、parser binary 3、doc
tests 2；Clippy 零告警、Rust fmt 与 diff check 通过；本轮未把工程测试冒充正式安装应用验收。

Slice 2 的历史产物授权也已与 Project 范围演进解耦。此前 Project 只要增加或删除一个额外只读
目录，revision 与 scope digest 就会变化，已经完成且摘要仍一致的产物也会失去提取预览、打开和
Undo；界面按文件事实判断可恢复，批准路径却按原始 Run revision 拒绝，形成前后端承诺冲突。现在
待执行的初始写入仍严格绑定原始 Project revision/scope，不能借后续扩权通过审批；已物化产物的
预览、打开、另存与受治理 Undo 则只接受当前主 Project 根或该 Conversation 的 canonical managed
root，并继续绑定精确 Artifact、Task/Run、路径、版本和内容摘要。额外只读目录变化不会再使历史
结果失效；主 Project 根换走后，旧文件的预览、打开和 Undo 会同步失败关闭，只有当前主根重新覆盖
该精确路径后才恢复。Undo 的批准、确认收据和启动 reconciliation 共用同一后物化授权函数，不能在
请求阶段通过、执行阶段又退回旧 revision。端到端回归覆盖“增加只读根仍可用 -> 主根换走全部拒绝
-> 主根恢复后批准 Undo”，并修正了一个把无 Project 产物伪造到 managed root 外部的旧测试夹具。
全量门禁为 Rust core 686 passed / 3 ignored、desktop 431 passed / 2 ignored、parser binary 3、
doc tests 2；前端 31 files / 274 tests、typecheck、format、production build 与 authority guard，
以及 Clippy、Rust fmt 和 diff check 全部通过。正式安装应用证据仍待后续原生验收。

该历史产物闭环现已补齐本地签名 QA 原生证据，并修正了原生操作中发现的跨读模型刷新缺口。
Project 主目录、额外只读目录、Task 与 PDF Artifact 在应用重启后均从持久化状态恢复；增加额外只读目录后，
既有 PDF 继续显示为已完成，提取预览、打开与另存入口保持可用。原先更换主目录只重读 Conversation，导致
结果区仍保留旧 Task 快照并要求用户再次点击“重新读取”；现在 Project 主目录绑定、增加只读目录和移除
只读目录都会在 Conversation 重读成功后同步重读同一对话的 Workbench，并清除可能钉住旧结果的前端选择。
修复后的原生复核证明：主目录换到不覆盖产物路径时，页面立即变为“缺少完成证据”并关闭预览、打开和修订；
切回原目录时立即恢复“已完成”、提取预览、打开、另存和继续修订，全程未手动刷新。QA 数据库同时确认
Project revision 及额外只读根已持久化，Artifact 的声明摘要、观察摘要和文件 SHA-256 完全一致。安装包使用
`ai.openlife.desktop.qa` 与 `OpenLife Local Code Signing`，built/staged/installed executable SHA-256 均为
`5f9e56e202f494a5ef0f5e57638a896f3f1e075af5a70bcdbfc4d4ac6c303a87`，strict codesign、Designated Requirement、
resource seal 和安装哈希一致性均通过；它仍是本地 QA 证据，不是 Developer ID、notarization 或正式发布验收。
前端回归更新为 31 files / 275 tests，并通过 typecheck、format、production build、authority guard 与 diff check；
Rust 全量门禁沿用本次后端变更后已通过的 686 passed / 3 ignored、desktop 431 passed / 2 ignored、parser 3、
doc tests 2、Clippy 与 Rust fmt。
