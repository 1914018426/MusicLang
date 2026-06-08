# MusicLang

MusicLang is a Rust-first experimental programming language for developing music with AI Agents. It treats music as explicit, checkable code: the Agent writes the music, while the compiler validates theory/style constraints, lowers to IR, renders multiple output formats, and exposes editor intelligence through LSP.

## What works

- Lexer/parser with spans for `.music` files.
- Statements for metadata, voices, notes, chords, drums, rests, generative figures, harmonic/melodic annotations, control flow, functions, local style scopes, and audited overrides.
- Typed expressions for integers, booleans, pitches, intervals, durations, and strings.
- Pitch arithmetic such as `C4 + M3` and `E4 - m3`.
- Algorithmic expression material with lists, dict event values, ranges, `at`, `len`, `with`/`merge`, `not`, and list comprehensions.
- Built-in style registry for `Classical`, `Modal`, `Jazz`, and `Minimalist` styles.
- Jazz quality gates for swing/syncopation identity, blues inflection, call-and-response writing, walking/riff bass support, predominant-dominant-tonic motion, authentic cadence, and pitch-domain counterpoint that excludes unpitched drum tracks.
- Theory-backed scale and mode constraints with `scale_pattern: tonic scale_id` and `mode_pattern: tonic mode_id`.
- Style checks for `scale`, `chord_vocab`, `chord_quality_vocab`, `set_class_vocab`, `meter`, `meter_catalog`, `tempo_range`, `rhythm_vocab`, `rhythm_concept`, `melodic_concept`, `phrase_concept`, `dynamic_vocab`, `articulation_vocab`, `ornament`, `non_chord_tone`, `tuning_system`, `world_tradition`, `historical_era`, `harmonic_function`, `max_melodic_leap`, `voice_spacing`, `contrapuntal_motion`, `cadence`, `harmonic_progression`, `texture`, `form`, `instrument_range`, `parallel_fifths`, and `voice_crossing`.
- Explicit local overrides with audit traces.
- IR metadata for tempo, meter, key signature, track name, channel, and program.
- MIDI rendering with tempo, time signature, key signature, track name, program change, and per-track channel.
- MusicXML rendering for notation interchange.
- WAV audio rendering for direct audition.
- CLI commands for compile/check/export/diagnose/ast/ir/theory/repl.
- LSP server with diagnostics, hover, completion, method-aware expression completions, signature help, semantic tokens, go-to-definition, references, document highlights, document symbols, inlay hints, formatting, folding ranges, rename, selection ranges, workspace symbols, and diagnostic quick-fix code actions.

## Install

```bash
cargo install --path crates/musiclang-cli
```

After installation, the user-facing compiler command is `music`:

```bash
music --version
```

## Developer build

```bash
cargo check --workspace
cargo test --workspace
```

Rust is only the implementation toolchain; MusicLang is distributed as its own language CLI.

## Project quickstart

```bash
music new demo_song
cd demo_song
music build
music build --manifest path/to/music.toml
```

A MusicLang project contains `music.toml`, `src/main.music`, and build outputs under `build/`. The manifest supports `name`, `source`, `output`, `format`, and `strict` keys with `#` comments. Set `strict = true` when project builds must reject every diagnostic without requiring `music build --strict`.

## CLI quickstart

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

`--strict` is the quality gate for publishable/listening material. It rejects every diagnostic, including warning-only style diagnostics, and rejects explicit suppression such as `override` blocks or `severity_*: off`. `music analyze --strict` also rejects excessive repeated bars. CI runs strict analysis and strict MIDI/MusicXML/WAV export smoke tests for the complete Jazz demo.

Listening demos are expected to pass without diagnostic suppression: no `override` for cleanup, no `severity_*: off`, no warnings, and no uncontrolled repeated-bar padding.

Run the REPL:

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

The extension contributes `.music` syntax highlighting, snippets, bracket/comment behavior, and an LSP client for diagnostics, hover, completion, signature help, navigation, formatting, rename, folding, selection ranges, semantic tokens, and quick fixes. Set `musiclang.serverPath` when the `musiclang-lsp` binary is not available at `target/debug/musiclang-lsp`, `target/release/musiclang-lsp`, or on `PATH`.

## Example

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

## Current examples

- `examples/algorithmic_expression.music`
- `examples/minimal.music`
- `examples/loop.music`
- `examples/control_flow.music`
- `examples/override.music`
- `examples/style_violation.music`
- `examples/custom_style.music`
- `examples/custom_style_violation.music`

See `docs/requirements/musiclang.md` for the broader product requirements.
