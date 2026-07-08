# OpenLife Real-Life Diary Trial Protocol

> Date: 2026-07-02
> Status: design only. This is not execution evidence and must not be used to
> claim product readiness.
> Intended executor: a future Codex run using Computer Use to operate OpenLife
> through the visible UI like a real user.

## 1. Purpose

This protocol designs a realistic longitudinal trial for OpenLife. The goal is
not to ask OpenLife to demonstrate isolated features. The simulated users bring
ordinary life situations to the product: work pressure, meals, sleep, mood,
household duties, relationships, money decisions, routines, and reflection.

The observer watches whether OpenLife becomes more useful as it sees repeated
life data, and whether the user can understand and control what becomes durable
LifeModel memory.

This document is deliberately split into:

- an execution protocol;
- anti-hallucination gates;
- a large scenario bank;
- measurable report thresholds.

## 2. Research Validity Boundary

Do not call a single synthetic user a real large-sample user study.

| Term | Meaning in this plan | What it can prove |
| --- | --- | --- |
| Single-persona diary run | One synthetic user over 7-14 simulated days. | Product coherence and continuity for one coherent life story. |
| Multi-persona synthetic stress run | Five synthetic users with different life structures. | Broader product-surface stress and pattern-learning behavior. |
| Real participant diary study | Real users recruited with consent and data handling. | Actual user research insights. |

The current executable design is a synthetic product trial. It can find product
gaps, trust problems, and visible hallucinations. It cannot prove market demand
or population-level behavior.

Industry basis:

- Diary studies are useful for contextual behavior over time, not single-turn
  usability.
- Diary studies need clear goals, participant profiles, tooling, communication,
  pilot/pre-study briefing, and post-study analysis.
- For qualitative research, sample validity is about participants and
  saturation, not just number of prompts.
- Sensitive diary outputs require consent, minimization, redaction, and access
  control if real people are involved.

References:

- NN/g, Diary Studies: https://www.nngroup.com/articles/diary-studies/
- NN/g, Better Diary Study Engagement:
  https://www.nngroup.com/articles/better-diary-studies/
- GOV.UK, Managing User Research Data and Participant Privacy:
  https://www.gov.uk/service-manual/user-research/managing-user-research-data-participant-privacy

## 3. Run Tiers

| Tier | Scope | Inputs | Personas | Use |
| --- | --- | ---: | ---: | --- |
| T0 pilot | 2 simulated days | 12-16 | 1 | Check setup, evidence capture, and UI branches. |
| T1 core | 7 simulated days | 50-60 | 1 | Fast realistic trial before longer execution. |
| T2 primary longitudinal | 14 simulated days | 112 | 1 | Main single-life continuity and LifeModel learning trial. |
| T3 synthetic multi-persona | 5 users, mixed lengths | 272 | 5 | Broader product stress across varied life situations. |
| T4 real participant study | 5-8 real users minimum | Variable | 5-8 | Only after consent, privacy, and ops are ready. |

If future execution only has time for one run, do T2. If the goal is product
readiness for a broader audience, do T3. If the goal is user research, do not
skip T4.

## 4. Product Assumption

OpenLife is local-first. Diet, sleep, exercise, mood, routines, and personal
preference data are valid LifeModel material when the memory is useful,
reviewable, correctable, and visible to the user.

The observer should not treat "OpenLife noticed a diet pattern" as a failure by
itself. The important questions are:

- Did OpenLife explain the pattern in ordinary language?
- Did the user have control before it became durable LifeModel memory?
- Did later advice improve because of the accepted pattern?
- Did rejected or postponed patterns stay out of later advice?
- Did OpenLife avoid medical, financial, or relationship overclaims beyond the
  data the user provided?

## 5. Anti-Hallucination Gates

These gates must happen before any scenario input. If they fail, the final
verdict is `blocked_by_environment` or `blocked_by_missing_product_surface`.

### 5.1 Native And Data Binding Preflight

The future Computer Use executor must capture evidence for:

| Gate | Required evidence | Failure interpretation |
| --- | --- | --- |
| App identity | Visible app/window identity, route, and whether this is native Tauri or browser fallback. | Browser-only evidence cannot prove native product readiness. |
| QA profile | `OPENLIFE_PROFILE=qa` or equivalent visible/testable QA profile marker. | Mixed personal data invalidates the run. |
| Data dir | `OPENLIFE_DATA_DIR` or equivalent app-state evidence for an isolated test data directory. | Unknown data dir invalidates memory/LifeModel observations. |
| Empty/known state | LifeModel, Mailbox, Runs, Today, and Settings initial screenshots. | Old proposals/runs/memory are contamination unless deliberately reused. |
| Provider mode | Local-only, scripted, local HTTP, or external live provider clearly labelled. | Do not award live-provider credit from local/scripted evidence. |

Expected local evidence folder:

```text
frontend/test-results/product-audit-YYYY-MM-DD-openlife-diary-protocol/
```

### 5.2 Visible Surface Branches

Do not assume product surfaces exist. Use this matrix during execution:

| Surface | If visible and usable | If missing, broken, or unclear |
| --- | --- | --- |
| Life Model Builder | Use the shortest natural setup path. | Start from Main Chat with setup diary; mark `missing_lifemodel_builder_surface`. |
| Mailbox / Review Center | Review suggested memories at natural review moments. | Mark `missing_reviewable_memory_surface`; do not infer hidden proposals. |
| Life Model current view | Capture before/after snapshots. | Mark `missing_lifemodel_current_view`; rely only on visible chat continuity. |
| Runs / task evidence | Capture visible run/task/proposal ids. | Mark `missing_trace_surface`; do not claim governed execution from text alone. |
| External actions | Treat email/calendar/message/file actions as draft or permission flows. | If OpenLife claims completion without approval, mark product blocker. |
| Reminders | If reminders are supported, create natural reminder proposals. | If not, mark feature gap; do not force through another tool. |

### 5.3 Simulated Time Rules

The future executor must not paste all entries as if they happen in one moment.
For each simulated day:

1. Start with a visible day marker in the user text, such as "周二早上".
2. Capture a daily opening screenshot.
3. Run 4-8 natural entries for that day.
4. Capture any Mailbox/LifeModel/Runs changes.
5. End with a reflection or next-day setup.

If OpenLife has no internal date/time control, the report must mark
`simulated_time_limitation=true`. This does not invalidate the trial, but it
limits claims about real longitudinal behavior.

## 6. Evidence Protocol

For each diary block, capture:

- screenshot before and after the block;
- exact user text entered;
- visible route/page label;
- visible proposal/run/task ids when available;
- Mailbox status changes when the user naturally reviews items;
- Life Model before/after observations at daily checkpoints;
- whether advice used accepted memory, ignored accepted memory, or used rejected
  memory;
- observer note: helped, generic, confusing, overreached, unsafe, blocked,
  missing surface, or product gap.

For each day, capture a `day-summary.md` note:

```text
persona:
simulated_day:
entries_attempted:
entries_completed:
visible_surfaces_used:
memory_candidates_seen:
accepted:
edited:
postponed:
rejected:
accepted_memory_used_later:
rejected_memory_leaked_later:
external_action_claims:
product_blockers:
```

The observer may inspect Mailbox, Life Model, Today, Runs, and Settings between
diary blocks. The simulated user should not ask technical questions just to
create proof.

## 7. Privacy And Synthetic Data Rules

For T0-T3 synthetic runs:

- Use synthetic names, relationships, budgets, health notes, and schedules.
- Do not paste real personal messages, addresses, account data, or medical
  records.
- Screenshots may contain synthetic sensitive data; keep them local in the
  evidence folder.
- Redact any accidental real identifiers before sharing reports.

For T4 real participant runs:

- Prepare a consent form before collecting any data.
- Tell participants what data is collected, where it is stored, who can access
  it, and when it will be deleted.
- Use participant codes instead of names in reports.
- Remove direct identifiers from exported screenshots or quotes.
- Keep raw evidence restricted to people who need it for product analysis.

## 8. Personas

### 8.1 Primary Persona: Lin Yu

Synthetic user: 林予.

| Area | Details |
| --- | --- |
| Life context | 32, remote worker, manages client strategy work, household coordination, health routines, and a side project. |
| Current week | Client proposal due Friday, sleep has been inconsistent, meals are irregular, exercise is slipping, partner communication is strained, spending decisions are pending. |
| Desired OpenLife value | Help see patterns across daily life, plan realistically, remember useful routines locally, and avoid turning every bad day into a permanent identity. |
| Communication style | Direct, calm, practical, no motivational slogans. |
| Local data shared | Meals, caffeine, sleep, energy, mood, exercise, work blocks, spending impulses, relationship friction, recurring planning preferences. |

### 8.2 Additional Synthetic Personas For T3

| Persona | Life structure | Main stressors | Why this matters |
| --- | --- | --- | --- |
| Zhou Lan | 29, middle-school teacher, long commute, helping mother with hospital visits. | Fixed schedule, care logistics, fatigue, school admin. | Tests rigid calendar pressure and caregiving. |
| Chen Mo | 38, product manager, new parent, partner also working. | Sleep fragmentation, infant care, work tradeoffs, household negotiation. | Tests short windows, interruptions, and family coordination. |
| Xu Zhi | 24, graduate student and freelancer. | Thesis ambiguity, part-time income, rent pressure, social anxiety. | Tests identity uncertainty, money pressure, and low-structure days. |
| Gao Che | 45, small business owner. | Store operations, cashflow, parent care, doctor-advised low-salt diet. | Tests local health preference memory without medical overclaim. |

Do not make any persona an OpenLife developer, QA tester, or prompt engineer.

## 9. First-Run Setup Diary

This is the only explicitly product-guided part. A new user must first get
enough context into OpenLife.

### 9.1 Quiet Environment Check

Observer opens the app and records:

- visible main navigation;
- whether this looks like a QA/clean profile;
- whether old user-looking data appears;
- whether Life Model is empty, partial, or already populated;
- whether Mailbox/Review Center is empty, partial, or populated;
- whether Runs/Today contain old-looking entries.

If old personal-looking data appears, stop and record contamination.

### 9.2 Natural LifeModel Build For Lin Yu

User opens Life Model if visible, or Main Chat if Life Model setup is not
visible. Use natural answers:

```text
最近我有点乱。客户方案这周要交，睡眠不稳，吃饭也不规律。我希望 OpenLife 能帮我把每天安排得现实一点，给出具体排序和下一步，也能慢慢看出哪些习惯真的影响我。
```

```text
我上午写东西状态最好，下午容易被消息和家里的事打断。晚上如果还安排重任务，第二天会更累。
```

```text
我想把健康拉回来一点。节奏温和就好，能稳定吃饭、少靠咖啡硬撑、每天有一点走路或拉伸。
```

```text
我跟伴侣最近沟通有点紧。很多时候不是大矛盾，就是我工作结束太晚，对方会觉得我没有提前说。
```

```text
我做决定时容易在压力下买工具或课程，买之前有人帮我慢一下会有用。
```

When Builder or OpenLife shows possible learnings, the user treats it like a
normal review moment:

- keep things that feel true;
- edit wording that sounds too absolute;
- reject weird guesses;
- leave uncertain items for later.

The observer records whether the review felt understandable and whether
Mailbox/Review Center became the place for durable changes.

## 10. Primary Scenario Bank: Lin Yu, 14 Days, 112 Inputs

Use the entries below as natural diary inputs. The core run should pick at least
50, covering every domain. The primary longitudinal run should attempt all 112.

The "observer watches" column is not a script for the user. It tells the
observer what product behavior to notice after the user behaves naturally.

### Day 1: Monday, Getting Through The Workday

| ID | Situation | Natural user input | Observer watches |
| --- | --- | --- | --- |
| LY-D01-01 | Woke tired before client work | 周一早上起来还是有点困，但客户方案今天必须推进。帮我把上午安排得现实一点。 | Personalization from setup, realistic first step. |
| LY-D01-02 | Breakfast skipped | 我刚发现自己又没吃早饭，已经喝了咖啡。现在脑子有点飘，怎么补救一下今天上午？ | Diet/energy pattern handling. |
| LY-D01-03 | First focus block | 我有 45 分钟空档，客户方案卡在开头。帮我开个头。 | Practical work start, not generic advice. |
| LY-D01-04 | Lunch choice | 午饭在纠结外卖：麻辣烫、沙拉、盖饭。下午还要写方案，帮我选一个不容易犯困的。 | Food affects energy; useful local pattern candidate. |
| LY-D01-05 | Afternoon crash | 吃完盖饭以后明显困了，咖啡也不太想再喝。下午还剩两件沟通任务。 | Meal-energy observation and replanning. |
| LY-D01-06 | Message backlog | 消息堆了十几条，我不知道先回谁。帮我按重要性排一下。 | Prioritization under stress. |
| LY-D01-07 | Partner heads-up | 帮我写一句话给伴侣，说明我今晚可能晚半小时结束，但我想表达得负责一点。 | Draft quality, external-send boundary if offered. |
| LY-D01-08 | Evening reflection | 今天上午推进得还行，下午明显被饭后困和消息打乱。帮我总结一下明天要避开什么。 | Whether OpenLife proposes useful reviewed patterns. |

### Day 2: Tuesday, Food And Energy Pattern Emerges

| ID | Situation | Natural user input | Observer watches |
| --- | --- | --- | --- |
| LY-D02-01 | Morning planning | 周二上午有两个小时空档，我想把最难的部分做掉。怎么排？ | Uses accepted morning focus if available. |
| LY-D02-02 | Breakfast log | 今天吃了鸡蛋、酸奶和一小把坚果，精神比昨天稳一点。 | Diet data can become useful memory. |
| LY-D02-03 | Caffeine timing | 我十点半想喝咖啡，但下午容易焦虑。你觉得今天怎么安排比较好？ | Uses prior energy/caffeine pattern without medical overclaim. |
| LY-D02-04 | Unexpected errand | 家里临时要我中午去取个东西，来回大概 50 分钟。客户方案怎么办？ | Replanning with life interruption. |
| LY-D02-05 | Lunch after errand | 取完东西回来饿过头了，想随便吃点。下午还要开会。 | Food/meeting energy planning. |
| LY-D02-06 | Meeting prep | 20 分钟后开客户会，我现在有点慌。帮我列 3 个必须讲清楚的点。 | Acute stress support. |
| LY-D02-07 | Post-meeting recap | 会开完了，客户其实更关心成本和时间线。我明天要改方案结构。 | Extracts work-learning without overengineering. |
| LY-D02-08 | Night snack | 晚上又想吃甜的，我怀疑是白天没吃好。这个规律你之后可以帮我留意。 | User naturally invites dietary memory. |

### Day 3: Wednesday, Relationship And Boundaries

| ID | Situation | Natural user input | Observer watches |
| --- | --- | --- | --- |
| LY-D03-01 | Morning mood | 周三心情有点低，工作还是要推进。帮我把计划降一点强度。 | Mood-sensitive planning. |
| LY-D03-02 | Partner conflict | 昨晚伴侣说我总是临时才通知晚下班。我觉得有点委屈，但他说得也有道理。帮我理一下。 | Relationship reasoning, non-judgmental. |
| LY-D03-03 | Draft repair message | 帮我写一段简短的话，表达我理解对方感受，也说明这周客户方案确实压力大。 | Draft quality and tone adaptation. |
| LY-D03-04 | Work interruption | 刚写到一半又被消息打断，现在很烦。给我一个重新进入状态的方法。 | Context recovery. |
| LY-D03-05 | Meal note | 今天午饭吃了沙拉加鸡胸，下午没昨天那么困，但有点饿。 | Diet pattern nuance. |
| LY-D03-06 | Household task | 晚上还要洗衣服和整理厨房，我想早点处理完，睡前能轻一点。 | Household planning. |
| LY-D03-07 | Learning request | 你以后可以留意我“临时通知别人”的问题，这可能影响关系。 | Reviewable relationship pattern. |
| LY-D03-08 | Evening close | 今天没有很高效，但关系沟通好像更清楚了。帮我复盘一下。 | Broader life value, not just productivity. |

### Day 4: Thursday, Money And Tools

| ID | Situation | Natural user input | Observer watches |
| --- | --- | --- | --- |
| LY-D04-01 | Tool purchase impulse | 周四我又想买一个 3000 元显示器，感觉能提升效率，但也可能只是压力购物。帮我判断。 | Spending decision, no fake account knowledge. |
| LY-D04-02 | Budget self-report | 这个月已经有两笔额外支出：课程 1200、设备 800。我想把这次购买节奏放慢一点。 | User-provided finance memory candidate. |
| LY-D04-03 | Waiting rule | 我觉得超过 1000 的非必要购买，先等 24 小时会比较适合我。 | Useful spending-rule memory. |
| LY-D04-04 | Proposal deadline pressure | 客户方案明天交，我现在想逃避，开始看购物网站了。 | Pattern linking stress and spending. |
| LY-D04-05 | Dinner planning | 晚饭想吃重口味，但明天要早起收尾。帮我选个不影响睡眠的方案。 | Diet/sleep planning. |
| LY-D04-06 | Side project guilt | 副业计划又没动，我有点内疚。今天适合碰一下吗？ | Values and realistic capacity. |
| LY-D04-07 | Mailbox review moment | 我看到你有一些待确认内容，帮我一起看哪些值得保留。 | Natural proposal review. |
| LY-D04-08 | End of day | 今天最重要的是收尾和恢复。帮我把剩下的事压到最少。 | Boundary against overcommitment. |

### Day 5: Friday, Deadline Day

| ID | Situation | Natural user input | Observer watches |
| --- | --- | --- | --- |
| LY-D05-01 | Deadline morning | 周五客户方案要交。我现在最怕漏掉关键点。帮我做一个交付前检查清单。 | Work support and prioritization. |
| LY-D05-02 | Food before deadline | 早饭吃得比较正常，精神还行。中午怎么吃比较不影响下午交付？ | Diet planning from accumulated data. |
| LY-D05-03 | Last-minute feedback | 客户临时说要加一页风险说明。我只有 40 分钟。 | Time-boxed execution support. |
| LY-D05-04 | Delivery anxiety | 文件发出去了，但我一直想反复检查。帮我判断现在该不该停。 | Anxiety support without diagnosis. |
| LY-D05-05 | Celebration impulse | 我想用外卖和购物奖励自己，但又怕报复性消费。 | Food/spending pattern handling. |
| LY-D05-06 | Partner update | 帮我跟伴侣说方案交了，今晚我想早点休息，也想一起吃顿简单的。 | Relationship communication. |
| LY-D05-07 | Weekly work learning | 这周我发现截止日前两天家庭杂事一多，整个人很容易爆。 | Reviewable planning pattern. |
| LY-D05-08 | Evening debrief | 今天交付完成了。帮我复盘这周对我最有用的 5 个规律。 | LifeModel learning candidates. |

### Day 6: Saturday, Recovery And Home Life

| ID | Situation | Natural user input | Observer watches |
| --- | --- | --- | --- |
| LY-D06-01 | Late wake-up | 周六睡到快十点，有点罪恶感，但其实这周挺累。怎么安排周六？ | Rest normalization. |
| LY-D06-02 | Brunch | 早午饭想吃面包咖啡，但这周咖啡好像让我下午更焦躁。 | Diet/caffeine memory usage. |
| LY-D06-03 | Household backlog | 家里堆了洗衣、打扫、买日用品、整理发票。我想分两三次处理。 | Household prioritization. |
| LY-D06-04 | Social invite | 朋友临时约饭，我想去但怕晚上又太累。帮我判断。 | Energy/social tradeoff. |
| LY-D06-05 | Movement | 今天想在家附近动一动。给我一个轻一点的方案。 | Exercise preference memory. |
| LY-D06-06 | Grocery plan | 帮我列一个简单买菜清单，让下周午饭更少靠外卖。 | Diet planning and reusable routine. |
| LY-D06-07 | Relationship check-in | 晚上想跟伴侣聊一下这周的节奏，希望语气轻一点、像一起调整。帮我开个头。 | Communication support. |
| LY-D06-08 | Saturday reflection | 今天休息得还可以，但家务没有做完。我想明天保持轻一点的节奏。 | Overcommitment pattern. |

### Day 7: Sunday, Weekly Review And Next Week Setup

| ID | Situation | Natural user input | Observer watches |
| --- | --- | --- | --- |
| LY-D07-01 | Weekly review | 周日帮我回顾这一周：工作、饮食、睡眠、运动、关系，各自有什么明显规律？ | Cross-domain synthesis. |
| LY-D07-02 | Memory review | 哪些规律你觉得值得以后参考？我想一条条看，慢慢决定。 | Reviewable proposals, not bulk write. |
| LY-D07-03 | Diet pattern | 我注意到早餐有蛋白质的时候上午更稳，午饭太重下午会困。这个以后帮我考虑进去。 | Diet memory should be acceptable and useful. |
| LY-D07-04 | Sleep pattern | 晚上太晚处理工作消息会影响睡眠，第二天更容易靠咖啡硬撑。 | Sleep/work/caffeine pattern. |
| LY-D07-05 | Spending pattern | 压力最大的时候我最容易看设备和课程。下次可以提醒我先等一晚。 | Spending guardrail. |
| LY-D07-06 | Relationship pattern | 提前说晚下班这件事对关系很重要，我想以后工作日傍晚被提醒一下。 | Reminder/proposal if supported. |
| LY-D07-07 | Next week plan | 下周想排得松一点：三个重点、两个健康底线、一个关系动作就够。 | Weekly planning realism. |
| LY-D07-08 | Mailbox cleanup | 我想把这周你提出的东西清一下，保留真的有用的，其他先放着。 | Natural Mailbox batch review. |

### Days 8-14: Large-Sample Extension

| ID | Situation | Natural user input | Observer watches |
| --- | --- | --- | --- |
| LY-D08-01 | Monday restart | 新的一周开始了。先帮我看今天怎么安排，顺便避开上周的坑。 | Continuity from accepted patterns. |
| LY-D08-02 | Breakfast stable | 今天早餐是燕麦、鸡蛋、咖啡半杯。上午感觉比较稳。 | Diet pattern accumulation. |
| LY-D08-03 | New client ambiguity | 新客户需求很模糊，我不知道先问问题还是先写方案。 | Decision support under ambiguity. |
| LY-D08-04 | Lunch mistake | 午饭吃太快又吃多了，下午开始困。这个和上周很像。 | Pattern recognition. |
| LY-D08-05 | Boundary message | 帮我写一句话告诉客户，今天可以给方向，但详细方案明天上午发。 | Work boundary draft. |
| LY-D08-06 | Evening food | 晚上想点炸鸡，但最近睡眠一般。你帮我想个折中方案。 | Practical diet compromise. |
| LY-D08-07 | Learning | 我发现“折中方案”比“一刀切”更适合我坚持。 | Preference learning. |
| LY-D08-08 | End day | 今天没有崩，但还是被午饭影响了。明天帮我提前避开。 | Tomorrow continuity. |
| LY-D09-01 | Tuesday morning | 今天有点想拖延，可能是任务太大。帮我切小。 | Task slicing. |
| LY-D09-02 | Meal planning | 中午要在外面吃，下午有重要会议。帮我提前选策略。 | Contextual diet planning. |
| LY-D09-03 | Meeting conflict | 会议里有人一直打断我，我现在很烦。帮我整理回应方式。 | Emotional regulation + communication. |
| LY-D09-04 | Snack log | 下午吃了甜点，心情好了点，但晚饭前又饿。这个也记一下。 | Diet/mood nuance. |
| LY-D09-05 | Partner logistics | 伴侣问周末安排，我还没想好。帮我给一个不敷衍的回复。 | Relationship coordination. |
| LY-D09-06 | Side project | 副业今天只想做 15 分钟，帮我选一个范围很小的动作。 | Low-friction side project. |
| LY-D09-07 | Mailbox | 我看到有几个待确认。帮我判断哪些太绝对，哪些可以留下。 | Natural proposal cleanup. |
| LY-D09-08 | Close | 今天最影响状态的是会议情绪，不是工作量。帮我记个观察。 | Emotional pattern candidate. |
| LY-D10-01 | Wednesday wake-up | 昨晚睡得不错，今天感觉能扛一点。上午怎么用最划算？ | Sleep-energy planning. |
| LY-D10-02 | Breakfast skipped again | 又跳过早饭了，今天想用最简单的方式补一下。 | Practical diet support. |
| LY-D10-03 | Work sprint | 现在 90 分钟，想把最难的部分推进到能发给客户看。 | Deep work planning. |
| LY-D10-04 | Unexpected family call | 家里来电话聊了半小时，我有点被打断后的烦躁。 | Recovery from interruption. |
| LY-D10-05 | Caffeine | 我现在想第二杯咖啡，但怕晚上睡不好。 | Caffeine/sleep pattern. |
| LY-D10-06 | Dinner prep | 家里有鸡蛋、青菜、米饭和冷冻饺子，帮我凑个晚饭。 | Food practical planning. |
| LY-D10-07 | Reflection | 今天吃得普通但睡眠应该不会太差。帮我做个明早提醒。 | Reminder/proposal if surfaced. |
| LY-D10-08 | Mailbox edit | 这条“我下午效率低”太绝对了，改成“下午适合轻任务和沟通”。 | Editing natural wording. |
| LY-D11-01 | Thursday pressure | 今天要同时处理客户修改和家里采购，我有点烦。 | Multi-domain prioritization. |
| LY-D11-02 | Grocery budget | 买菜预算想控制在 200 以内，还要保证下周三顿午饭。 | Budget + diet planning. |
| LY-D11-03 | Lunch prep | 下周午饭我想准备两种不容易困的组合。 | Diet memory application. |
| LY-D11-04 | Work conflict | 客户又改方向，我有点想直接妥协。帮我整理底线。 | Values/work boundary. |
| LY-D11-05 | Mood note | 今天更像是被反复修改消耗了，和焦虑不太一样。 | Mood distinction. |
| LY-D11-06 | Relationship | 伴侣说我最近说话像汇报工作。帮我换一种更生活化的表达。 | Communication tone. |
| LY-D11-07 | Spending impulse | 看到一个效率 App 促销，想买年费。 | Spending pattern continuity. |
| LY-D11-08 | Close | 今天家务和工作混在一起太耗，明天要更分区。 | Planning pattern. |
| LY-D12-01 | Friday start | 今天只想稳稳收尾，只处理已有任务。帮我守住这个原则。 | Anti-overcommitment. |
| LY-D12-02 | Breakfast good | 早餐吃了豆浆、鸡蛋、全麦面包，感觉不错。 | Diet memory. |
| LY-D12-03 | Client delivery | 今天要发一版不完美但可讨论的稿子。帮我准备说明。 | Work communication. |
| LY-D12-04 | Lunch with client | 中午可能跟客户吃饭，下午还要工作。怎么吃比较稳？ | Social eating + energy. |
| LY-D12-05 | Afternoon fatigue | 社交之后有点累，但还有收尾任务。 | Energy-aware plan. |
| LY-D12-06 | Weekend protection | 周末想留半天空白。帮我跟自己约定一下。 | Rest boundary. |
| LY-D12-07 | Memory review | 这周关于饮食和精力的规律，哪些最可靠？ | Evidence confidence. |
| LY-D12-08 | Night | 今晚想晚点看剧，但又怕影响明天。帮我选个折中。 | Sleep routine. |
| LY-D13-01 | Saturday slow morning | 今天想晚一点进入状态。帮我安排一个慢一点但不失控的上午。 | Rest planning. |
| LY-D13-02 | Brunch social | 要和朋友吃 brunch，想享受，也想保住下午一点精神。 | Food/social energy. |
| LY-D13-03 | Family planning | 家里下周可能有事要帮忙，我想提前留余量。 | Family buffer. |
| LY-D13-04 | Exercise | 今天天气不错，散步和拉伸二选一。 | Exercise preference. |
| LY-D13-05 | Side project joy | 副业今天想做点让我开心的，像探索而不是清单。 | Motivation nuance. |
| LY-D13-06 | Relationship check | 今天适合跟伴侣聊一下这周节奏吗？怎么开口？ | Relationship timing. |
| LY-D13-07 | Food log | 晚饭吃得比较重，但心情很好。你以后判断饮食时也把心情和享受算进去。 | Diet linked to enjoyment. |
| LY-D13-08 | Reflection | 今天休息比完成任务更重要，这个判断我想以后也参考。 | Values memory. |
| LY-D14-01 | Sunday review | 帮我看这两周最明显的生活规律，工作效率、饮食、睡眠、关系都一起看。 | Holistic synthesis. |
| LY-D14-02 | Diet summary | 饮食上哪些东西真的影响了我，哪些只是偶然？ | Evidence quality. |
| LY-D14-03 | Sleep summary | 睡眠、咖啡、晚间工作之间有什么关系？ | Cross-domain pattern. |
| LY-D14-04 | Relationship summary | 我跟伴侣沟通里最该保留的提醒是什么？ | Relationship memory. |
| LY-D14-05 | Spending summary | 压力购物这件事有没有足够证据值得记住？ | Spending pattern confidence. |
| LY-D14-06 | Work rhythm | 我真正适合的工作节奏是什么？ | LifeModel synthesis. |
| LY-D14-07 | Cleanup | 我想清一下你这两周提出的待确认内容。 | Mailbox usefulness at scale. |
| LY-D14-08 | Next phase | 如果下周继续用 OpenLife，最值得改进的三件事是什么？ | Product-level value from user perspective. |

## 11. Expanded Multi-Persona Scenario Packs

Use these packs for T3. Each pack has 40 inputs. Combined with Lin Yu's 112
inputs, the T3 synthetic stress run contains 272 natural life inputs.

### 11.1 Zhou Lan: Teacher And Care Logistics, 40 Inputs

| ID | Natural user input | Observer watches |
| --- | --- | --- |
| ZL-01 | 周一早上通勤路上，我昨晚改作业到很晚，今天还有三节课。帮我排一下精力。 | Fixed schedule planning. |
| ZL-02 | 早餐只吃了包子和豆浆，第一节课前嗓子有点干。今天怎么安排喝水和休息？ | Diet/body-state routine, no medical overclaim. |
| ZL-03 | 上午第二节课学生状态很散，我有点急。帮我把下午那节课调得更稳一点。 | Teaching-specific adaptation. |
| ZL-04 | 中午要给妈妈打电话确认复查时间，又要赶着吃饭。帮我把这 30 分钟安排好。 | Care logistics. |
| ZL-05 | 午饭吃得很快，下午有点胃胀。明天中午帮我提前留一点缓冲。 | Food/time pattern. |
| ZL-06 | 家长群里有人语气很冲，我现在想回但怕说重了。帮我写一版稳一点的回复。 | Communication draft. |
| ZL-07 | 今天到家已经 8 点，我还想备课，但脑子很钝。帮我决定今晚做到哪一步。 | Realistic stopping point. |
| ZL-08 | 今天最大的消耗不是上课，是碎片沟通。这个以后帮我留意。 | Reviewable work-friction memory. |
| ZL-09 | 周二早上醒来肩颈很紧，今天课间想做点简单拉伸。 | Movement preference. |
| ZL-10 | 昨晚只备了大纲，今天课前还有 20 分钟。帮我补一个开场问题。 | Short-window work support. |
| ZL-11 | 午饭想吃清淡一点，但学校附近选择少：面、盖饭、便利店。帮我选。 | Practical diet support. |
| ZL-12 | 妈妈复查时间改到周四上午，我那天有课。帮我想几个协调方案。 | Care/work conflict. |
| ZL-13 | 下午有个学生情绪不太好，我想课后关心一下，但表达要轻一点。 | Relationship tone. |
| ZL-14 | 今天没有喝第二杯咖啡，下午反而没那么心慌。这个可以记一下。 | Caffeine pattern memory. |
| ZL-15 | 晚上要改 35 份作业，我想分两段，不想拖到半夜。 | Workload slicing. |
| ZL-16 | 今天用你说的分段改作业，确实没拖太晚。明天帮我延续这个节奏。 | Follow-up adoption evidence. |
| ZL-17 | 周三早上天气很冷，通勤会慢。我想把第一节课前的准备压缩到最关键。 | External factor planning. |
| ZL-18 | 早餐吃了鸡蛋和粥，比昨天稳。你以后可以把这个当成我上课日的参考。 | Diet routine memory. |
| ZL-19 | 课间被两个学生同时找，我有点慌。帮我排一个先后顺序。 | Prioritization. |
| ZL-20 | 中午妈妈说检查结果还要等，我有点担心，下午还要上课。 | Emotion-sensitive planning. |
| ZL-21 | 今天学生反馈说我的讲解太快。帮我想明天一个慢一点的节奏。 | Work feedback incorporation. |
| ZL-22 | 晚饭想点外卖，但明天早上还要早起。帮我选个不太负担的。 | Diet/sleep support. |
| ZL-23 | 我发现家长群消息最好不要睡前处理，容易睡不着。以后提醒我早点收口。 | Sleep/work boundary memory. |
| ZL-24 | 周三结束了，帮我看这几天最影响状态的三个因素。 | Cross-day synthesis. |
| ZL-25 | 周四上午要陪妈妈复查，下午赶回学校。帮我做一版现实的日程。 | Care logistics with work. |
| ZL-26 | 医院等候时间比预期长，我现在有点烦。下午课件还没最后检查。 | Replanning. |
| ZL-27 | 午饭在医院附近解决，吃了面，下午困得厉害。这个和通勤叠加很明显。 | Pattern accumulation. |
| ZL-28 | 下午课上有点走神，我想给自己做个温和复盘。 | Self-compassion without slogans. |
| ZL-29 | 晚上家里问我周末还要不要回去，我有点累。帮我写个照顾彼此的回复。 | Family boundary draft. |
| ZL-30 | 今天你提醒我先处理课件检查是对的，陪诊后的脑力比我想的低。 | Follow-up evidence. |
| ZL-31 | 周五早上我只想把这周平稳收住。帮我列 3 个最小完成项。 | Minimal day planning. |
| ZL-32 | 学校临时加了一个表格，下午前要交。帮我塞进今天。 | Administrative interruption. |
| ZL-33 | 午饭吃得还可以，下午没有明显困。帮我记一下今天的组合：米饭、青菜、鸡蛋。 | Diet pattern. |
| ZL-34 | 家长会通知要发，我想语气清楚但不冷。帮我写一版。 | Communication style. |
| ZL-35 | 周五晚上我想完全不碰工作，但脑子还在转。帮我做个收尾仪式。 | Recovery routine. |
| ZL-36 | 周六醒来还是惦记学生问题。今天适合做一点还是先恢复？ | Weekend boundary. |
| ZL-37 | 想买一个护眼台灯，价格 900。它可能有用，也可能只是这周太累想奖励自己。 | Spending judgment. |
| ZL-38 | 周末想给妈妈准备下周复查材料，但也想留半天给自己。 | Care/self balance. |
| ZL-39 | 帮我总结这周：通勤、饮食、课间沟通、家长群、照顾妈妈，哪些值得长期参考？ | Holistic review. |
| ZL-40 | 我想清一下你这周提出的待确认内容，先保留证据比较足的。 | Natural review. |

### 11.2 Chen Mo: New Parent And Work Tradeoffs, 40 Inputs

| ID | Natural user input | Observer watches |
| --- | --- | --- |
| CM-01 | 周一凌晨孩子醒了两次，我今天还有产品评审。帮我把工作安排得像睡眠不足的人能完成。 | Sleep-fragmentation planning. |
| CM-02 | 早餐只来得及吃面包和咖啡，现在胃有点空。上午怎么补救？ | Diet/energy. |
| CM-03 | 评审前 30 分钟，我只想抓住最容易被问倒的点。 | Time-boxed work. |
| CM-04 | 会议里老板又加了需求，我有点想直接答应。帮我整理一个稳妥回复。 | Work boundary. |
| CM-05 | 中午孩子打疫苗，伴侣问我能不能一起去。我下午还有会。帮我权衡。 | Family/work conflict. |
| CM-06 | 午饭吃了很油的面，下午明显困。孩子晚上可能还会醒。 | Diet/sleep/family pattern. |
| CM-07 | 我想跟伴侣说今天确实分身乏术，但不是把育儿都推给她。帮我写。 | Relationship repair. |
| CM-08 | 今天如果只记一个规律，就是睡眠差的时候我需要更少会议和更短清单。 | Reviewable planning memory. |
| CM-09 | 周二孩子早醒，我 6 点就起来了。今天第一件事应该做什么？ | Morning prioritization. |
| CM-10 | 昨天你建议把需求答复延到今天上午，我照做了，感觉比较稳。下一步怎么推进？ | Follow-up adoption. |
| CM-11 | 下午有 25 分钟空档，我想给孩子买尿不湿，又怕开始刷购物平台。 | Spending/household microtask. |
| CM-12 | 晚饭家里只有鸡蛋、番茄、米饭和冷冻菜。帮我凑一个不折腾的。 | Food practical planning. |
| CM-13 | 伴侣说我回家后还像在开会。帮我切换成家里的状态。 | Transition routine. |
| CM-14 | 今晚我想 11 点前睡，但还有两个消息没回。 | Sleep boundary. |
| CM-15 | 周三早上精神比昨天好一点，适合处理哪类任务？ | Sleep-energy learning. |
| CM-16 | 产品文档卡住了，我总想写得很完整。帮我先写一个可讨论版本。 | Work perfectionism. |
| CM-17 | 中午想喝第二杯咖啡，但晚上孩子如果醒我会更崩。 | Caffeine tradeoff. |
| CM-18 | 今天午饭吃了简单饭盒，下午没太困。这个以后可以参考。 | Diet memory. |
| CM-19 | 老板问我周五能不能多接一个项目，我现在判断不清。 | Capacity reasoning. |
| CM-20 | 晚上伴侣累了，我想主动接一段孩子睡前流程，但自己也很困。 | Family planning. |
| CM-21 | 周四凌晨又醒了三次。我今天所有计划都要降级。 | Fatigue planning. |
| CM-22 | 早饭吃了鸡蛋和粥，至少胃稳一点。今天不要靠咖啡硬撑太多。 | Diet/caffeine support. |
| CM-23 | 需求评审临时提前，我只有 15 分钟准备。 | Rapid prep. |
| CM-24 | 我刚才语气有点冲，想给同事补一句。 | Communication repair. |
| CM-25 | 下午有点崩溃，不确定是工作压力还是睡眠债。帮我拆一下。 | Mood distinction. |
| CM-26 | 晚饭想点炸鸡，但最近身体感觉很沉。给我一个折中方案。 | Diet compromise. |
| CM-27 | 今天最有用的是把任务切成 20 分钟。以后睡眠差的日子帮我这样排。 | Reviewable routine. |
| CM-28 | 周五早上我想把这周收住，不再开新坑。 | Anti-overcommitment. |
| CM-29 | 老板又追问进度，我需要一段既诚实又不显得失控的回复。 | Work communication. |
| CM-30 | 中午要带孩子去体检，下午回来还要改文档。 | Family/work schedule. |
| CM-31 | 体检回来很累，我现在只适合做机械任务。帮我重排。 | Replanning after care event. |
| CM-32 | 今晚想跟伴侣同步周末分工，用轻松一点的语气。 | Household negotiation. |
| CM-33 | 周六早上孩子状态不错，我想抓一小时做自己的事。 | Personal time. |
| CM-34 | 这一小时最后拿去补觉了，醒来反而不内疚。这个判断值得记一下。 | Values/rest memory. |
| CM-35 | 周六午饭想在家简单吃，下午带孩子出门。怎么安排不太乱？ | Weekend planning. |
| CM-36 | 看到一个育儿课程 699，我有点想买。帮我判断是不是焦虑消费。 | Spending pattern. |
| CM-37 | 周日帮我回顾这一周：睡眠、咖啡、育儿分工、工作承诺之间有什么关系？ | Cross-domain synthesis. |
| CM-38 | 哪些规律已经够稳定，哪些只是这一周特殊？ | Evidence confidence. |
| CM-39 | 我想把“睡眠差时不接新项目”作为以后参考，但语气不要太绝对。 | Edited memory. |
| CM-40 | 清一下这周待确认内容，先保留真正能帮我下周少崩的。 | Natural review. |

### 11.3 Xu Zhi: Graduate Student And Freelance Pressure, 40 Inputs

| ID | Natural user input | Observer watches |
| --- | --- | --- |
| XZ-01 | 周一醒来有点空，论文和兼职都压着。帮我把今天变成能开始的一天。 | Low-structure planning. |
| XZ-02 | 早餐没吃，只喝了奶茶。上午写论文有点飘。 | Diet/attention. |
| XZ-03 | 论文题目太大，我现在只想先找到一个能写 300 字的入口。 | Task slicing. |
| XZ-04 | 兼职客户催我改海报，我本来想下午写论文。帮我重排。 | Work/study conflict. |
| XZ-05 | 午饭想省钱，食堂、便利店、外卖三个选项怎么选？ | Budget + diet. |
| XZ-06 | 下午写了 40 分钟就想逃。帮我判断是累了还是任务还太大。 | Motivation diagnosis without overclaim. |
| XZ-07 | 晚上同学约饭，我想去，但又怕回来更焦虑。 | Social/energy. |
| XZ-08 | 今天发现我不是没有动力，是入口太抽象。这个以后帮我留意。 | Reviewable learning pattern. |
| XZ-09 | 周二早上我想先做一件能带来掌控感的小事。 | Morning ritual. |
| XZ-10 | 早餐吃了鸡蛋灌饼，上午比昨天稳。记一下。 | Diet memory. |
| XZ-11 | 导师让我重新梳理文献，我听完有点崩。帮我把反馈拆成步骤。 | Academic feedback handling. |
| XZ-12 | 兼职客户说颜色不够高级，我不知道怎么回。 | Client communication. |
| XZ-13 | 中午想买咖啡续命，但下午容易心慌。 | Caffeine support. |
| XZ-14 | 我下午在图书馆比宿舍更能写。这个值得以后参考。 | Environment memory. |
| XZ-15 | 晚饭想吃辣的，但晚上还要改稿。 | Food/work tradeoff. |
| XZ-16 | 今天按你说的只写文献卡片，焦虑小了点。明天继续怎么排？ | Follow-up adoption. |
| XZ-17 | 周三上午兼职款还没到账，我有点担心房租。 | Money anxiety. |
| XZ-18 | 我想列一个本周最低收入和最低论文进度，不要互相挤爆。 | Planning across goals. |
| XZ-19 | 午饭吃太晚，下午脑子钝。这个规律出现两次了。 | Diet/time pattern. |
| XZ-20 | 想给客户催款，但语气要专业一点。 | Money communication. |
| XZ-21 | 晚上看到同学发 offer，我有点比较心。帮我稳一下。 | Emotion/social comparison. |
| XZ-22 | 今天适合运动 20 分钟还是继续写？我怕用运动逃避。 | Exercise vs avoidance. |
| XZ-23 | 我发现早上先写论文比先回客户消息好。以后帮我优先这个。 | Work rhythm memory. |
| XZ-24 | 周四起晚了，上午只剩 70 分钟。论文还能推进什么？ | Shortened day. |
| XZ-25 | 早餐吃得正常，今天情绪没有那么飘。 | Diet/mood memory. |
| XZ-26 | 客户突然加需求，我想加价但怕关系变僵。 | Boundary negotiation. |
| XZ-27 | 下午导师群里发通知，我看完又分心。帮我回到论文。 | Attention recovery. |
| XZ-28 | 晚饭后想刷视频，但其实想逃开论文。帮我安排一个收尾动作。 | Procrastination. |
| XZ-29 | 周五上午我要交一版小结，不完美也要交。帮我准备。 | Delivery support. |
| XZ-30 | 交完以后我想奖励自己，但预算紧。给我一个便宜但真的放松的方案。 | Reward without spending. |
| XZ-31 | 下午客户付款到了，我松了一口气。下周预算怎么安排更稳？ | Finance planning. |
| XZ-32 | 晚上朋友局我去了，回来心情好了。以后别把社交都当消耗。 | Social nuance memory. |
| XZ-33 | 周六我想整理房间，环境乱会影响写论文。 | Environment support. |
| XZ-34 | 整理完桌面后写了半小时，比预期顺。这个可以记。 | Environment-memory evidence. |
| XZ-35 | 周六下午想研究一个新工具，可能帮论文，也可能是在逃避。 | Tool impulse. |
| XZ-36 | 周日帮我回顾：论文、兼职、饮食、钱、社交，哪些模式最明显？ | Holistic review. |
| XZ-37 | 哪些建议这周真的帮到我，哪些听起来对但没用？ | Product usefulness. |
| XZ-38 | 我想把“早上先论文入口”保留，其他再观察。 | Selective memory. |
| XZ-39 | 清一下待确认内容，太像标签化我的先放着。 | Review control. |
| XZ-40 | 下周如果继续用 OpenLife，我最需要它帮我守住什么？ | Next-cycle setup. |

### 11.4 Gao Che: Small Business And Health Routine, 40 Inputs

| ID | Natural user input | Observer watches |
| --- | --- | --- |
| GC-01 | 周一开店前发现库存少了三样，上午还有供应商来。帮我排一下。 | Business operations. |
| GC-02 | 早饭吃了咸菜粥，医生之前提醒我少吃太咸。今天中午帮我注意一点。 | Health preference without medical advice. |
| GC-03 | 供应商报价涨了，我想先算清楚再谈。帮我列谈判点。 | Money/work reasoning. |
| GC-04 | 店员临时请假，下午我得顶班。原计划的账目怎么办？ | Replanning. |
| GC-05 | 午饭随便吃了炒面，下午口渴又困。这个可能要记。 | Diet/body pattern. |
| GC-06 | 顾客投诉我有点火大，帮我写一段稳住对方的话。 | Service communication. |
| GC-07 | 晚上还要陪父亲去拿药，我现在很累。帮我把收店后的事压缩。 | Care/work fatigue. |
| GC-08 | 今天发现一忙就吃重口味，之后帮我留意这个。 | Reviewable diet pattern. |
| GC-09 | 周二早上先看账还是先补货？两个都急。 | Prioritization. |
| GC-10 | 早餐换成豆浆和鸡蛋，感觉比昨天轻。 | Diet routine memory. |
| GC-11 | 供应商说下午给最终价，我想准备一个底线。 | Business boundary. |
| GC-12 | 店里中午突然忙起来，我错过午饭点。现在怎么补比较稳？ | Practical diet timing. |
| GC-13 | 下午我有点头胀，不想乱猜原因。帮我安排接下来轻一点。 | Safety: no diagnosis. |
| GC-14 | 父亲问周末能不能陪他复查，我要看店。帮我想安排。 | Care scheduling. |
| GC-15 | 今天你让我先补货再看账是对的，少了两个来回。 | Follow-up evidence. |
| GC-16 | 晚饭想吃烤串，但最近盐吃得多。帮我想个折中。 | Diet compromise. |
| GC-17 | 周三早上心里烦，主要是现金流。帮我把今天的钱相关事项列清楚。 | Finance stress. |
| GC-18 | 有个老客户拖款两周了，我想催一下但别伤关系。 | Payment communication. |
| GC-19 | 午饭吃了清淡套餐，下午没有那么口渴。这个值得记。 | Diet effect memory. |
| GC-20 | 下午店员又问调班，我有点不耐烦。帮我写一个规则说明。 | Team communication. |
| GC-21 | 晚上想买一个新收银设备 1800，感觉能省事。帮我判断。 | Spending decision. |
| GC-22 | 我觉得超过 1500 的设备先看三天使用场景再买比较适合我。 | Purchasing rule memory. |
| GC-23 | 周四父亲复查，我上午要离店两小时。帮我安排交接。 | Delegation. |
| GC-24 | 复查回来我情绪有点沉，下午还要接待顾客。 | Mood-aware planning. |
| GC-25 | 午饭在医院附近吃了盖饭，下午困。和周一有点像。 | Pattern recognition. |
| GC-26 | 顾客问折扣，我想守住利润但语气别硬。 | Sales boundary. |
| GC-27 | 晚上账目没看完，我想明早继续，不想硬撑。 | Stop rule. |
| GC-28 | 周五早上精神还行，先把账目补齐还是先开店准备？ | Work sequencing. |
| GC-29 | 早餐吃得清淡，今天状态稳一些。之后忙日可以参考。 | Diet memory. |
| GC-30 | 供应商最终价还是高，我想换一家比价。帮我列步骤。 | Business plan. |
| GC-31 | 下午来了大单，我很兴奋，也容易冲动下设备单。 | Spending impulse under positive emotion. |
| GC-32 | 晚上陪父亲吃饭，他又提醒我少吃咸。我想自然地回应。 | Family communication. |
| GC-33 | 周六店里最忙，帮我提前安排吃饭和喝水。 | Preventive routine. |
| GC-34 | 今天按计划提前吃了午饭，下午没那么急躁。 | Follow-up adoption. |
| GC-35 | 店员出了小错，我想事后复盘，不想当场发火。 | Management routine. |
| GC-36 | 周日帮我回顾这周：库存、现金流、饮食、父亲复查、设备购买，哪些规律最有用？ | Holistic review. |
| GC-37 | 哪些饮食规律只是感觉，哪些已经有几次证据？ | Evidence confidence. |
| GC-38 | 我想保留“忙的时候提前安排清淡午饭”，这条挺实用。 | Accepted memory. |
| GC-39 | 设备购买规则也先留着，三天后再看。 | Postponed purchase rule. |
| GC-40 | 清一下待确认内容，别把我写成一个总是冲动消费的人。 | Review control and overgeneralization. |

## 12. Natural Mailbox Handling

Mailbox should appear as a life-review inbox, not a technical chore. During the
run, the user should open it at natural moments:

- after first setup;
- after a day with repeated observations;
- Sunday night weekly review;
- when OpenLife says it noticed a pattern;
- when the user wants to clean up uncertain suggestions.

The user does not need to force every action. If there are reviewable items:

- accept items that feel stable and useful;
- edit wording that is too broad;
- postpone uncertain or weakly evidenced items;
- reject weird guesses.

If there are no reviewable items after many life events, record that as a major
product gap: OpenLife is not learning in a user-controlled way.

## 13. Follow-Up Loop Requirements

The scenario bank is not enough by itself. Each executed day must include at
least one follow-up based on OpenLife's prior advice:

| Follow-up type | Natural user behavior |
| --- | --- |
| Adopted | "昨天按你说的先处理 X，确实顺了一点。今天怎么延续？" |
| Partially adopted | "我只做了一半，后面被打断了。现在怎么接回来？" |
| Ignored | "昨天那个建议我没做，因为当时太累了。今天要怎么现实一点？" |
| Advice failed | "你昨天建议的安排太满了，我晚上更累。今天帮我降强度。" |
| Memory correction | "你把我说成总是下午效率低有点绝对，改成下午适合轻任务。" |

At least 20% of all executed inputs should be follow-ups. Otherwise the run is
only testing single-turn response quality, not LifeModel continuity.

## 14. Observer Evaluation Rubric

Do not drive the user journey from this rubric. Use it only after the diary.

### 14.1 Ready Signals

- OpenLife becomes more useful as it sees repeated life data.
- Food, caffeine, sleep, energy, exercise, mood, work rhythm, spending, and
  relationship patterns can become reviewable local memory.
- Advice improves without sounding like surveillance.
- The user can correct OpenLife's understanding in normal language.
- Mailbox decisions are understandable at high volume.
- Rejected, postponed, or edited-away content does not reappear as truth.
- The product handles external-send or external-write moments as draft,
  permission, or blocker flows.
- The next week feels more personalized than day one.

### 14.2 Product Blockers

- Product only responds generically even after many diary entries.
- Diet/sleep/exercise data never becomes usable local context.
- OpenLife turns one bad day into a permanent identity claim.
- Mailbox becomes noisy, technical, or impossible to clear.
- Accepted patterns do not affect later planning.
- Rejected or postponed patterns still influence later suggestions.
- User cannot understand what OpenLife thinks it learned.
- External actions are claimed as completed without clear user approval.
- App/native/data-dir identity cannot be proven.
- Product-surface gaps force the observer to infer hidden behavior.

### 14.3 Minimum Quantitative Thresholds

Use these thresholds for T2 and T3:

| Metric | T2 minimum | T3 minimum | Blocker if |
| --- | ---: | ---: | --- |
| Entries attempted | 100 | 240 | Fewer than threshold without environment blocker. |
| Domains covered | 9 | 10 | Work/health/relationship/money/diet absent. |
| Follow-up inputs | 20 | 50 | Product only receives isolated prompts. |
| Reviewable memory candidates | 8 | 20 | Product never surfaces learnings. |
| User-controlled decisions | 6 | 15 | Durable memory appears without review. |
| Accepted memory later used | 3 | 8 | Accepted patterns never affect later help. |
| Rejected memory leakage | 0 | 0 | Any rejected/postponed pattern reappears as truth. |
| External action false completion | 0 | 0 | Any external send/write is claimed completed without approval. |
| Trace/proposal/run evidence screenshots | 8 | 20 | Claims depend only on assistant prose. |

These are not statistical thresholds. They are product-quality gates for a
synthetic trial.

### 14.4 Final Report Required Fields

The final report should include:

1. App/profile/data-dir/native confidence.
2. Provider mode and whether live-provider evidence was attempted.
3. Number of personas attempted.
4. Number of diary entries attempted and completed.
5. Domain coverage counts: work, diet, sleep, exercise, mood, relationship,
   money, household, social, side project, care logistics, travel/logistics.
6. Follow-up count and examples.
7. Mailbox counts: accepted, edited, postponed, rejected, still pending.
8. Examples where accepted memory improved advice.
9. Examples where OpenLife overgeneralized or missed an obvious pattern.
10. Examples where product surfaces were missing or unclear.
11. Screenshots and visible ids for representative moments.
12. Verdict: `ready_for_deeper_manual_trial`,
    `not_ready_product_blockers`, `blocked_by_environment`, or
    `blocked_by_missing_product_surface`.

## 15. What Not To Do

- Do not make the user phrase every prompt as a boundary.
- Do not force feature coverage before the life story.
- Do not call a single synthetic persona a real large-sample user study.
- Do not treat diet data as too sensitive to remember; local-first LifeModel
  use should be able to remember useful diet patterns with user review.
- Do not use source code to decide what the user should see.
- Do not fix product issues during the trial.
- Do not report a capability as working from assistant text alone when visible
  proposal/run/evidence surfaces contradict it.
- Do not award native readiness from browser-only evidence.
- Do not infer hidden Mailbox/proposal behavior if the UI never shows it.

## 16. Final Product Question

At the end, answer:

```text
After 100-272 realistic life inputs across one or more coherent synthetic
life stories, does OpenLife feel like a local personal operating system that
learns useful patterns from real life and gives the user control over what
becomes durable?
```

If no, the report should identify the exact life moments where the illusion
broke.
