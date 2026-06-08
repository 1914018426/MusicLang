# MusicLang

## English

MusicLang is a Rust-first experimental programming language for developing music with AI agents. It treats music as explicit, checkable code: the agent writes the music, while the compiler validates theory and style constraints, lowers source into IR, renders multiple output formats, and exposes editor intelligence through LSP.

### What works

- Lexer/parser with spans for `.music` files.
- Statements for metadata, voices, notes, chords, drums, rests, generative figures, harmonic/melodic annotations, control flow, functions, local style scopes, and audited overrides.
- Typed expressions for integers, booleans, pitches, intervals, durations, strings, lists, tuples, dict event values, ranges, comprehensions, and transform builtins.
- Pitch arithmetic such as `C4 + M3` and `E4 - m3`.
- Built-in style registry for `Classical`, `Modal`, `Jazz`, and `Minimalist` styles.
- Jazz quality gates for swing/syncopation identity, blues inflection, call-and-response writing, walking/riff bass support, predominant-dominant-tonic motion, authentic cadence, and pitch-domain counterpoint that excludes unpitched drum tracks.
- Theory-backed scale and mode constraints with `scale_pattern: tonic scale_id` and `mode_pattern: tonic mode_id`.
- Style checks for scale, chord vocabulary, chord quality, set class, meter, tempo, rhythm, dynamics, articulation, ornaments, non-chord tones, tuning systems, world traditions, historical eras, harmonic function, melodic leap, voice spacing, contrapuntal motion, cadence, harmonic progression, texture, form, instrument range, parallel fifths, and voice crossing.
- Explicit local overrides with audit traces.
- IR metadata for tempo, meter, key signature, track name, channel, program, volume, pan, articulations, markers, and source spans.
- MIDI rendering with tempo, time signature, key signature, track name, program change, channel volume, pan, and per-track channel.
- MusicXML rendering for notation interchange.
- WAV audio rendering for direct audition.
- CLI commands for project creation, build, compile, check, export, diagnose, analyze, AST/IR inspection, theory catalog lookup, idiom lookup, style listing, format listing, and REPL.
- REPL commands for loading source, switching style, diagnostics, export, IR/source display, theory/style/format/idiom lookup, async playback launch, playback stop, reset, and quit.
- LSP server with diagnostics, hover, completion, method-aware expression completions, signature help, semantic tokens, go-to-definition, references, document highlights, document symbols, inlay hints, formatting, folding ranges, rename, selection ranges, workspace symbols, and diagnostic quick-fix code actions.
- VS Code extension with `.music` syntax highlighting, snippets, language configuration, and an LSP client.

### Install

```bash
cargo install --path crates/musiclang-cli
```

After installation, the user-facing compiler command is `music`:

```bash
music --version
```

### Developer build

```bash
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Rust is only the implementation toolchain; MusicLang is distributed as its own language CLI.

### Project quickstart

```bash
music new demo_song
cd demo_song
music build
music build --manifest path/to/music.toml
```

A MusicLang project contains `music.toml`, `src/main.music`, and build outputs under `build/`. The manifest supports `name`, `source`, `output`, `format`, and `strict` keys with `#` comments. Set `strict = true` when project builds must reject every diagnostic without requiring `music build --strict`.

### CLI quickstart

```bash
music check examples/minimal.music
music check examples/demo_jazz_complete.music --strict
music analyze examples/demo_jazz_complete.music --strict
music analyze examples/demo_jazz_complete.music --json
music compile examples/demo_jazz_complete.music -o /tmp/musiclang-jazz.mid --strict
music export examples/minimal.music --format midi -o /tmp/musiclang-minimal.mid
music export examples/minimal.music --format musicxml -o /tmp/musiclang-minimal.musicxml
music export examples/minimal.music --format wav -o /tmp/musiclang-minimal.wav
music export examples/demo_jazz_complete.music --format wav -o /tmp/musiclang-jazz.wav --strict
music diagnose examples/style_violation.music --json
music ast examples/minimal.music
music ir examples/minimal.music
music styles
music styles --json
music formats
music formats --json
music idioms
music idioms --json
music theory --domain scales
music theory --domain dynamics
music theory --domain harmonic_functions
music theory --domain scales --json
music theory --find maqam
music theory --find maqam --json
```

`--strict` is the quality gate for publishable/listening material. It rejects every diagnostic, including warning-only style diagnostics, and rejects explicit suppression such as `override` blocks or `severity_*: off`. `music analyze --strict` also rejects excessive repeated bars.

Listening demos are expected to pass without diagnostic suppression: no `override` for cleanup, no `severity_*: off`, no warnings, and no uncontrolled repeated-bar padding.

### REPL

```bash
music repl
```

Inside the REPL:

```text
:load examples/override.music
:style Classical
:diagnose
:styles
:formats
:idioms
:theory scales
:play
:stop
:show source
:show ir
:export /tmp/demo.mid
:reset
:quit
```

`:play` renders the current REPL buffer to a temporary MIDI file and starts `MUSICLANG_PLAYER` asynchronously when that environment variable points to a MIDI player executable. Without `MUSICLANG_PLAYER`, it still prints the rendered `.mid` path for manual audition; `:stop` stops the active player process.

### LSP and VS Code

Run the LSP server over stdio:

```bash
cargo run -q -p musiclang-lsp
```

Use the VS Code extension from this workspace:

```bash
npm ci --prefix editors/vscode
npm run --prefix editors/vscode compile
code --extensionDevelopmentPath "$PWD/editors/vscode"
```

The extension contributes `.music` syntax highlighting, snippets, bracket/comment behavior, and an LSP client. Set `musiclang.serverPath` when the `musiclang-lsp` binary is not available at `target/debug/musiclang-lsp`, `target/release/musiclang-lsp`, or on `PATH`.

### Example

```musiclang
style Classical
score override_demo {
  tempo 96
  meter 4/4
  key C major
  voice lead {
    program 40
    note C4, 1/4
    note C4 + M3, 1/4
    override scale allow reason "intentional chromatic color" {
      note F#4, 1/4
    }
    chord [C4, E4, G4], 1/2
  }
}
```

### Current examples

- `examples/algorithmic_expression.music`
- `examples/minimal.music`
- `examples/loop.music`
- `examples/control_flow.music`
- `examples/override.music`
- `examples/style_violation.music`
- `examples/custom_style.music`
- `examples/custom_style_violation.music`
- `examples/drum_groove.music`
- `examples/demo_classical_minuet.music`
- `examples/demo_jazz_blues.music`
- `examples/demo_jazz_complete.music`
- `examples/demo_minimal_pulse.music`
- `examples/demo_cinematic_ambient.music`

See `docs/language-reference.md` for the language reference and `docs/requirements/musiclang.md` for the broader product requirements.

---

## 中文

MusicLang 是一门 Rust 优先实现的实验性音乐编程语言，用于让 AI Agent 开发音乐。它把音乐视为显式、可检查的代码：Agent 负责编写音乐，编译器负责验证乐理与风格约束、降低到 IR、渲染多种输出格式，并通过 LSP 提供编辑器智能能力。

### 当前能力

- 为 `.music` 文件提供带 span 的 lexer/parser。
- 支持 metadata、voice、note、chord、drum、rest、生成式乐句、和声/旋律标注、控制流、函数、局部 style 作用域和带审计记录的 override。
- 支持整数、布尔、音高、音程、时值、字符串、列表、元组、dict event、range、列表推导和 transform builtin 等 typed expressions。
- 支持 `C4 + M3`、`E4 - m3` 等音高运算。
- 内置 `Classical`、`Modal`、`Jazz`、`Minimalist` 风格注册表。
- Jazz 风格质量门禁覆盖 swing/syncopation identity、blues inflection、call-response、walking/riff bass、predominant-dominant-tonic motion、authentic cadence，以及排除无音高鼓轨的 pitch-domain counterpoint。
- 支持基于理论目录的 `scale_pattern: tonic scale_id` 与 `mode_pattern: tonic mode_id` 约束。
- 风格检查覆盖音阶、和弦词表、和弦质量、集合类、拍号、速度、节奏、力度、奏法、装饰音、非和声音、调律系统、世界传统、历史风格、和声功能、旋律跳进、声部间距、对位运动、终止式、和声进行、织体、曲式、乐器音域、平行五度和声部交叉。
- 支持显式局部 override，并在 IR 中保留审计 trace。
- IR 包含 tempo、meter、key signature、track name、channel、program、volume、pan、articulation、marker 和 source span 等元数据。
- MIDI 渲染支持 tempo、time signature、key signature、track name、program change、channel volume、pan 和逐轨 channel。
- 支持用于记谱软件交换的 MusicXML 渲染。
- 支持用于快速试听的 WAV 音频渲染。
- CLI 支持新建项目、build、compile、check、export、diagnose、analyze、AST/IR 查看、理论目录查询、idiom 查询、style 列表、format 列表和 REPL。
- REPL 支持加载源码、切换 style、诊断、导出、查看 IR/source、查询 theory/style/format/idiom、异步启动播放、停止播放、reset 和 quit。
- LSP server 支持 diagnostics、hover、completion、method-aware expression completions、signature help、semantic tokens、go-to-definition、references、document highlights、document symbols、inlay hints、formatting、folding ranges、rename、selection ranges、workspace symbols 和 diagnostic quick-fix code actions。
- VS Code 扩展提供 `.music` 语法高亮、snippets、语言配置和 LSP client。

### 安装

```bash
cargo install --path crates/musiclang-cli
```

安装后，面向用户的编译器命令是 `music`：

```bash
music --version
```

### 开发构建

```bash
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Rust 只是实现工具链；MusicLang 以独立语言 CLI 的形式提供。

### 项目快速开始

```bash
music new demo_song
cd demo_song
music build
music build --manifest path/to/music.toml
```

一个 MusicLang 项目包含 `music.toml`、`src/main.music`，并把构建产物写入 `build/`。manifest 支持 `name`、`source`、`output`、`format` 和 `strict` 字段，也支持 `#` 注释。设置 `strict = true` 后，`music build` 会默认拒绝任何诊断，而无需额外传入 `music build --strict`。

### CLI 快速开始

```bash
music check examples/minimal.music
music check examples/demo_jazz_complete.music --strict
music analyze examples/demo_jazz_complete.music --strict
music analyze examples/demo_jazz_complete.music --json
music compile examples/demo_jazz_complete.music -o /tmp/musiclang-jazz.mid --strict
music export examples/minimal.music --format midi -o /tmp/musiclang-minimal.mid
music export examples/minimal.music --format musicxml -o /tmp/musiclang-minimal.musicxml
music export examples/minimal.music --format wav -o /tmp/musiclang-minimal.wav
music export examples/demo_jazz_complete.music --format wav -o /tmp/musiclang-jazz.wav --strict
music diagnose examples/style_violation.music --json
music ast examples/minimal.music
music ir examples/minimal.music
music styles
music styles --json
music formats
music formats --json
music idioms
music idioms --json
music theory --domain scales
music theory --domain dynamics
music theory --domain harmonic_functions
music theory --domain scales --json
music theory --find maqam
music theory --find maqam --json
```

`--strict` 是面向发布/试听材料的质量门禁。它会拒绝所有诊断，包括 warning-only style diagnostics，也会拒绝 `override` block 或 `severity_*: off` 这类显式 suppression。`music analyze --strict` 还会拒绝过度重复的小节。

试听 demo 应在没有诊断 suppression 的情况下通过：不允许为了清理而使用 `override`，不允许 `severity_*: off`，不应有 warnings，也不应通过失控的重复小节填充内容。

### REPL

```bash
music repl
```

REPL 内可用命令：

```text
:load examples/override.music
:style Classical
:diagnose
:styles
:formats
:idioms
:theory scales
:play
:stop
:show source
:show ir
:export /tmp/demo.mid
:reset
:quit
```

`:play` 会把当前 REPL buffer 渲染到临时 MIDI 文件；当 `MUSICLANG_PLAYER` 环境变量指向 MIDI 播放器可执行文件时，它会异步启动播放器。未配置 `MUSICLANG_PLAYER` 时，`:play` 仍会打印生成的 `.mid` 路径，方便手动试听；`:stop` 会停止当前活跃播放器进程。

### LSP 与 VS Code

通过 stdio 启动 LSP server：

```bash
cargo run -q -p musiclang-lsp
```

从当前 workspace 使用 VS Code 扩展：

```bash
npm ci --prefix editors/vscode
npm run --prefix editors/vscode compile
code --extensionDevelopmentPath "$PWD/editors/vscode"
```

该扩展提供 `.music` 语法高亮、snippets、括号/注释行为和 LSP client。如果 `musiclang-lsp` binary 不在 `target/debug/musiclang-lsp`、`target/release/musiclang-lsp` 或 `PATH` 中，可以设置 `musiclang.serverPath`。

### 示例

```musiclang
style Classical
score override_demo {
  tempo 96
  meter 4/4
  key C major
  voice lead {
    program 40
    note C4, 1/4
    note C4 + M3, 1/4
    override scale allow reason "intentional chromatic color" {
      note F#4, 1/4
    }
    chord [C4, E4, G4], 1/2
  }
}
```

### 当前示例

- `examples/algorithmic_expression.music`
- `examples/minimal.music`
- `examples/loop.music`
- `examples/control_flow.music`
- `examples/override.music`
- `examples/style_violation.music`
- `examples/custom_style.music`
- `examples/custom_style_violation.music`
- `examples/drum_groove.music`
- `examples/demo_classical_minuet.music`
- `examples/demo_jazz_blues.music`
- `examples/demo_jazz_complete.music`
- `examples/demo_minimal_pulse.music`
- `examples/demo_cinematic_ambient.music`

语言规范见 `docs/language-reference.md`，更完整的产品需求见 `docs/requirements/musiclang.md`。
