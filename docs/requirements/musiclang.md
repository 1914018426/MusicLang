# MusicLang 技术需求文档

> 状态：Draft | Author：Claude | Date：2026-06-07

## 1. 背景与目标

### 背景

MusicLang 是一门面向 vibe coding 时代的音乐编程语言。它不是用于开发传统软件，而是用于让 AI Agent 以显式、可检查、可调试的方式开发音乐。

当前 AI 音乐生成工具通常偏黑盒：用户输入提示词后得到音频结果，但难以精确控制旋律、和声、节奏、风格约束和局部修改。MusicLang 的目标是把音乐创作转化为一种可编程、可类型检查、可实时反馈的开发过程。

参考文档：`ScoreLang_项目介绍与规划说明.md`。该文档仅作为参考，当前项目名称确定为 MusicLang。

### 核心理念

- **音乐即代码**：旋律、和声、节奏、曲式、风格和乐器编排都应能被显式表达。
- **乐理即类型**：Pitch、Interval、Chord、Duration、Voice、Style 等音乐概念是语言内置类型。
- **Agent 是主要作者**：AI Agent 负责编写 MusicLang 代码，用户提供创作意图并判断结果。
- **编译器只检查，不代替创作**：编译器负责类型检查、乐理检查、风格约束检查和输出渲染，不自动生成音乐内容。
- **风格是类型上下文**：音乐风格不是普通 prompt，而是影响类型检查和合法性的上下文。
- **允许显式突破规则**：默认违反风格约束应报错，但用户/Agent 可以通过局部 override 显式突破。

### 目标

| ID | 目标 | 成功标准 |
|----|------|----------|
| G-01 | 构建一门 Rust 优先实现的音乐编程语言 | 能解析 MusicLang 源码，生成 IR，并输出 MIDI |
| G-02 | 支持 REPL 交互式音乐开发 | Agent 能在 REPL 中输入代码并快速听到修改结果 |
| G-03 | 建立风格驱动的类型约束系统 | 声明 style 后，编译器能根据风格规则检查音乐代码 |
| G-04 | 支持局部 override | 违反风格约束时默认报错，但可用显式语法局部放行 |
| G-05 | MIDI 优先输出，同时保留多后端扩展 | MVP 支持 MIDI；架构预留 MusicXML、音频、实时播放后端 |

### 非目标

- MVP 不做黑盒音乐生成模型。
- MVP 不让编译器自动补全、自动和声化或自动作曲。
- MVP 不要求完整覆盖所有人类乐理知识，但架构必须支持逐步扩展到大型乐理知识库。
- MVP 不优先开发完整 IDE 插件；REPL 和 CLI 优先。
- MVP 不追求专业级音频合成质量；MIDI 可听、可导入 DAW 优先。

## 2. 用户故事

### 主要用户角色

1. **AI Agent 作曲者**
   - 读取用户意图。
   - 编写 MusicLang 代码。
   - 根据编译器错误修改代码。
   - 通过 REPL 快速试听和迭代。

2. **研究/演示用户**
   - 观察语言如何表达音乐。
   - 观察风格类型约束如何工作。
   - 评估 Agent 是否能显式、可控地开发音乐。

3. **人类创作者/设计者**
   - 定义风格配置。
   - 指定允许或禁止的乐理规则。
   - 审阅 Agent 生成的代码和音乐结果。

### 用户故事

| ID | 用户故事 | 优先级 |
|----|----------|--------|
| US-01 | As an Agent, I want to write notes, chords, durations, and voices explicitly, so that the music is controllable and debuggable. | P0 |
| US-02 | As an Agent, I want to declare a music style, so that the compiler checks my code against the selected style rules. | P0 |
| US-03 | As an Agent, I want to use variables, functions, loops, and conditions, so that I can develop music structurally rather than manually repeating events. | P0 |
| US-04 | As a user, I want to hear changes quickly in a REPL, so that I can guide the Agent through iterative music development. | P0 |
| US-05 | As an Agent, I want to locally override style constraints, so that intentional rule-breaking is explicit and reviewable. | P0 |
| US-06 | As a user, I want MIDI output, so that generated music can be played and imported into a DAW. | P0 |
| US-07 | As a researcher, I want the system to support multiple and custom styles, so that MusicLang can model diverse musical traditions over time. | P1 |
| US-08 | As a user, I want future MusicXML/audio output, so that MusicLang can support notation and direct listening workflows. | P2 |

### 关键用户旅程：局部 override Demo

1. 用户要求 Agent 创作一段指定风格的音乐。
2. Agent 在 REPL 中声明全局 style，例如 `style Classical`。
3. Agent 编写旋律、和声、节奏与声部结构。
4. 编译器根据 Classical 风格类型上下文检查代码。
5. Agent 故意写入一处违反风格规则的音乐片段。
6. 编译器默认报错并指出位置、规则和原因。
7. Agent 使用局部 override 显式声明该片段为例外或切换局部风格。
8. 编译器通过检查，生成 MIDI。
9. 用户听到音乐并看到代码中可解释的规则突破点。

## 3. 功能需求

### 3.1 核心功能

| ID | 功能描述 | 优先级 | 验收标准 |
|----|----------|--------|----------|
| F-01 | 定义基础音乐类型：Pitch、Interval、Duration、Note、Chord、Voice、Score | P0 | 可以声明并组合基础音乐对象；非法基础值产生类型错误 |
| F-02 | 支持音高和音程运算 | P0 | `C4 + M3` 等表达式能得到正确音高；非法运算报错 |
| F-03 | 支持时值与时间线 | P0 | 多个 Note/Chord 能按顺序或并行映射到绝对时间 |
| F-04 | 支持变量、函数、循环、条件 | P0 | 可用控制流生成重复乐句、变奏和条件片段 |
| F-05 | 支持全局 style 声明 | P0 | 源码可声明当前音乐风格；类型检查器能读取风格上下文 |
| F-06 | 支持配置式风格规则 | P0 | 风格可由 scale、chord、rhythm、range、voice-leading 等配置字段描述 |
| F-07 | 支持风格驱动类型约束 | P0 | 违反当前风格硬约束时默认编译失败 |
| F-08 | 支持局部 override | P0 | 局部代码块可显式关闭、替换或放宽某些风格规则 |
| F-09 | 支持 REPL | P0 | Agent 可输入 MusicLang 片段并触发解析、检查、渲染和试听/输出 |
| F-10 | 支持 MIDI 输出 | P0 | 通过 CLI/REPL 可输出可播放 `.mid` 文件 |
| F-11 | 提供清晰错误信息 | P0 | 错误包含行列、规则名、当前 style、错误原因和建议的显式处理方式 |
| F-12 | 支持多个内置风格的扩展框架 | P1 | 新增风格不需要修改 parser，只需添加风格配置与规则实现 |
| F-13 | 支持 MusicXML 输出 | P2 | 可输出基础乐谱结构，用于导入记谱软件 |
| F-14 | 支持音频输出 | P2 | 可通过 SoundFont 或外部合成器从 MIDI 渲染音频 |
| F-15 | 支持不中断实时播放 | P2 | 代码变更可影响后续播放片段，不要求 MVP 实现 |

### 3.2 详细规则

#### 3.2.1 风格声明

MusicLang 应支持至少三种风格作用域：

1. **全局风格**：作用于整个 Score 或 REPL session。
2. **局部风格**：作用于某个 block、voice、section 或 phrase。
3. **自定义风格**：通过配置式规则定义新的风格上下文。

示意语法仅供需求表达，最终语法待设计：

```musiclang
style Classical {
  scale: major(C)
  harmony: functional
  parallel_fifths: error
}

score demo {
  voice soprano {
    note C4, 1/4
    note E4, 1/4
  }

  override parallel_fifths: allow {
    chord [C4, G4], 1/2
  }
}
```

#### 3.2.2 风格约束优先级

- 默认违反当前 style 的类型约束应报错。
- 局部 override 必须显式写出，不允许静默放行。
- override 应在 IR 或诊断报告中保留痕迹，便于用户审查。
- 某些基础乐理错误不应被风格 override 放行，例如无法解析的音高、负数时值、无效 MIDI 范围等。

#### 3.2.3 配置式自定义风格

MVP 的自定义风格以配置式为主，不要求用户写任意检查函数。

候选字段：

| 字段 | 含义 |
|------|------|
| `scale` | 允许或偏好的音阶/调式 |
| `chord_vocab` | 允许或偏好的和弦集合 |
| `progression` | 和声进行规则 |
| `rhythm_patterns` | 允许或偏好的节奏型 |
| `meter` | 拍号与重音结构 |
| `tempo_range` | 推荐速度范围 |
| `instrument_ranges` | 乐器音域约束 |
| `voice_leading` | 声部连接规则 |
| `texture` | 单声部、复调、主调、循环层等织体约束 |
| `strictness` | 规则严格程度或 error/warn/off 策略 |

#### 3.2.4 “集成所有人类乐理知识”的落地方式

长期愿景是尽可能容纳人类已有乐理体系，但不应在 MVP 中一次性实现。

推荐分层：

1. **Core Theory Kernel**：音高、音程、时值、和弦、音阶、节拍、声部、时间线。
2. **Western Tonal Pack**：大小调、功能和声、常见终止式、基础声部连接。
3. **Jazz Pack**：七和弦/扩展和弦、调式、ii-V-I、替代和弦。
4. **Electronic/Rhythm Pack**：pattern、loop、drum grid、swing、syncopation。
5. **Atonal/Experimental Pack**：集合理论、序列、非功能和声、不规则节奏。
6. **World/Microtonal Packs**：非十二平均律、微分音、地域性调式与节奏系统。
7. **User-defined Pack**：项目级自定义风格规则。

MVP 应至少完成第 1 层，并选择 1-2 个风格包做端到端演示。

## 4. 非功能需求

### 性能

| ID | 要求 | 优先级 |
|----|------|--------|
| NFR-01 | REPL 中小型片段从输入到诊断结果应在 500ms 内完成 | P0 |
| NFR-02 | 30-60 秒 MIDI 片段生成应在 2 秒内完成 | P0 |
| NFR-03 | 类型检查架构应支持未来增量编译 | P1 |

### 安全与可控性

| ID | 要求 | 优先级 |
|----|------|--------|
| NFR-04 | MusicLang 代码不应默认执行宿主系统命令 | P0 |
| NFR-05 | REPL 不应允许任意文件系统写入，除显式导出路径外 | P0 |
| NFR-06 | 自定义风格 MVP 采用配置式，避免执行不可信用户代码 | P0 |

### 可用性

| ID | 要求 | 优先级 |
|----|------|--------|
| NFR-07 | 错误信息应面向 Agent 修复，结构化且可复制进上下文 | P0 |
| NFR-08 | CLI/REPL 命令应简单，例如 compile、play、export、diagnose | P0 |
| NFR-09 | 示例应覆盖局部 override、风格错误、MIDI 导出 | P0 |

### 可扩展性

| ID | 要求 | 优先级 |
|----|------|--------|
| NFR-10 | Parser、AST、IR、Type Checker、Renderer 应模块化 | P0 |
| NFR-11 | 新增输出后端不应影响类型系统 | P1 |
| NFR-12 | 新增风格包不应修改核心语法 | P1 |

## 5. 技术方案

### 5.1 架构概览

推荐采用 Rust 编译器优先路线，同时提供 REPL 入口。

```text
MusicLang Source / REPL Input
        ↓
Lexer / Parser
        ↓
AST
        ↓
Name Resolution + Scope Context
        ↓
Type Checker
        ├─ Core Music Type Rules
        ├─ Style Context Rules
        ├─ Override Validation
        └─ Diagnostics
        ↓
Musical IR
        ├─ Absolute Timeline
        ├─ Voices / Tracks
        ├─ Instruments
        └─ Style Metadata
        ↓
Render Backends
        ├─ MIDI Renderer   P0
        ├─ MusicXML        P2
        ├─ Audio Renderer  P2
        └─ Realtime Player P2
```

### 5.2 核心模块

| 模块 | 职责 |
|------|------|
| `parser` | 词法分析、语法分析、AST 构建 |
| `ast` | 源码结构表达 |
| `types` | Pitch、Interval、Duration、Chord、Style 等类型定义 |
| `theory` | 乐理计算与基础规则 |
| `style` | 风格配置、风格上下文、规则选择 |
| `checker` | 类型检查、风格约束检查、override 检查 |
| `ir` | 可渲染的音乐中间表示 |
| `midi` | MIDI 文件输出 |
| `repl` | 交互式输入、状态管理、诊断展示 |
| `cli` | 命令行入口 |

### 5.3 数据模型

#### Pitch

| 字段 | 示例 | 说明 |
|------|------|------|
| `class` | C, D#, Bb | 音级/变音 |
| `octave` | 4 | 八度 |
| `midi_number` | 60 | 可选规范化表示 |
| `tuning` | 12-TET | 长期扩展字段 |

#### Duration

| 字段 | 示例 | 说明 |
|------|------|------|
| `ratio` | 1/4 | 音符时值 |
| `ticks` | 480 | MIDI/IR 内部表示 |

#### Note

| 字段 | 示例 | 说明 |
|------|------|------|
| `pitch` | C4 | 音高 |
| `duration` | 1/4 | 时值 |
| `velocity` | 80 | MIDI 力度 |
| `articulation` | staccato | 长期扩展 |

#### StyleContext

| 字段 | 说明 |
|------|------|
| `name` | 风格名称 |
| `rules` | 当前启用的规则集合 |
| `severity` | error/warn/off 策略，MVP 默认 error |
| `parent` | 继承来源，可选 |
| `overrides` | 局部覆盖记录 |

### 5.4 接口定义

#### CLI

```bash
music compile input.music -o output.mid
music repl
music diagnose input.music
music export input.music --format midi
```

#### REPL 命令

```text
:style Classical
:load examples/override.music
:play
:export demo.mid
:diagnose
:reset
```

REPL 状态模型仍待最终确认，候选方案：

1. 累积状态：适合作曲过程。
2. 单次片段：实现简单。
3. 双模式：默认累积，同时支持临时试听片段。

当前建议：MVP 采用双模式，但需用户最终确认。

### 5.5 关键技术选型

| 选型 | 推荐 | 理由 | Trade-off |
|------|------|------|-----------|
| 实现语言 | Rust | 性能、类型安全、适合编译器工程 | 初期迭代比 Python 慢 |
| 路线 | 编译器优先 + REPL | 符合语言定位，同时满足交互演示 | REPL 状态设计复杂 |
| 输出 | MIDI 优先 | 易实现、可播放、可导入 DAW | 音色表现有限 |
| 自定义风格 | 配置式 | 安全、可检查、易序列化 | 灵活性低于代码式规则 |
| 风格约束 | 类型约束 | 项目核心创新明确 | 需要清晰设计 Style Type System |

## 6. 实施计划

### M1：语言核心与 MIDI 输出

产出物：

- Rust workspace 初始化。
- Parser/AST 基础实现。
- Pitch、Interval、Duration、Note、Chord、Voice、Score 类型。
- 基础音高/音程/时值检查。
- Musical IR。
- MIDI 输出。
- 1-2 个最小示例。

验收：

- `.music` 文件可编译为 `.mid`。
- 非法 Pitch/Duration 会报错。
- 30 秒以内简单旋律可播放。

### M2：控制流与 REPL

产出物：

- 变量、函数、循环、条件。
- REPL 输入与诊断。
- REPL 播放/导出命令。
- Agent 可用的示例 prompt 与 few-shot。

验收：

- Agent 可通过循环生成重复乐句。
- REPL 中可以快速修改并重新输出 MIDI。

### M3：风格类型系统与局部 override

产出物：

- 全局 style 声明。
- 配置式风格定义。
- 风格驱动类型检查。
- 默认违反规则报错。
- 局部 override 语法与检查。
- Demo：局部 override 成功流程。

验收：

- 同一段代码在不同 style 下可产生不同检查结果。
- 违反当前 style 默认编译失败。
- 加入显式 override 后可通过并输出 MIDI。

### M4：乐理知识库扩展框架

产出物：

- Core Theory Kernel 完整化。
- 风格包注册机制。
- 至少 1-2 个内置风格包。
- 规则文档和测试用例。

验收：

- 新增风格包无需修改 parser。
- 每个规则有对应测试。

## 7. 验收与测试

### 功能测试用例

| ID | 场景 | 预期结果 |
|----|------|----------|
| T-01 | 声明 `Pitch C4` | 类型检查通过 |
| T-02 | 声明非法音高 | 编译失败并定位错误 |
| T-03 | 执行 `C4 + M3` | 得到 E4 或等价内部表示 |
| T-04 | 使用 for 循环生成 4 个音符 | IR 中出现 4 个事件 |
| T-05 | 声明全局 style | Checker 进入对应风格上下文 |
| T-06 | 违反 style 硬约束 | 默认编译失败 |
| T-07 | 用 override 包裹违规片段 | 编译通过，并记录 override 元数据 |
| T-08 | 导出 MIDI | 文件可被标准播放器或 DAW 打开 |
| T-09 | REPL 中修改片段后播放 | 输出反映最新代码 |

### Demo 验收流程

1. 启动 `music repl`。
2. 声明一个风格。
3. Agent 输入一段带控制流的 MusicLang 代码。
4. 编译器报出一个风格约束错误。
5. Agent 用局部 override 显式处理该错误。
6. 编译通过。
7. 导出并播放 MIDI。
8. 展示诊断报告中保留了 override 记录。

## 8. 开放问题

| ID | 问题 | 当前建议 |
|----|------|----------|
| Q-01 | REPL 状态模型采用累积、单次还是双模式？ | 建议双模式：默认累积，支持临时片段试听 |
| Q-02 | MVP 第一批内置风格包具体选哪些？ | 建议 Core Theory + Classical 或 Core Theory + Electronic，避免一次性实现全部 |
| Q-03 | MusicLang 语法是否类 Rust、类 Python，还是自定义声明式语法？ | 需要下一轮专门设计 |
| Q-04 | 风格类型约束的最小规则集合是什么？ | 建议从 scale、chord_vocab、meter、instrument_range 开始 |
| Q-05 | MIDI 播放由内置播放器、系统播放器还是仅导出文件完成？ | MVP 可先导出文件，REPL 调用外部播放器待定 |
| Q-06 | 是否支持多风格叠加冲突解决？ | 建议 MVP 只支持嵌套覆盖，不做复杂合并 |
| Q-07 | override 是否需要理由字段？ | 建议支持可选 reason，便于研究展示和 Agent 审查 |

## 附录

### A. 参考哲学来自 ScoreLang

- 乐理即类型。
- 显式操作。
- 编译器只检查。
- 创作自由。
- Guard/Style 规则可选、可覆盖。

### B. 推荐下一步

1. 确定 MusicLang 语法风格。
2. 确定 REPL 状态模型。
3. 定义 Core Theory Kernel 的最小类型和规则。
4. 设计 style/override 的最小语法。
5. 生成架构设计文档与 MVP 任务拆分。
