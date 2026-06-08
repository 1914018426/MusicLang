# MusicLang

MusicLang is a Rust-first experimental programming language for developing music with AI Agents. It treats music as explicit, checkable code: the Agent writes the music, while the compiler validates theory/style constraints, lowers to IR, renders multiple output formats, and exposes editor intelligence through LSP.

## What works

- Lexer/parser with spans for `.music` files.
- Statements: `style`, `score`, `tempo`, `meter`, `key`, `voice`, `program`, `note`, `chord`, `for`, `if`, `let`, `fn`, `call`, and `override`.
- Typed expressions for integers, booleans, pitches, intervals, durations, and strings.
- Pitch arithmetic such as `C4 + M3` and `E4 - m3`.
- Built-in style registry for `Classical`, `Modal`, `Jazz`, and `Minimalist` styles.
- Theory-backed scale and mode constraints with `scale_pattern: tonic scale_id` and `mode_pattern: tonic mode_id`.
- Style checks for `scale`, `chord_vocab`, `chord_quality_vocab`, `set_class_vocab`, `meter`, `meter_catalog`, `tempo_range`, `rhythm_vocab`, `rhythm_concept`, `dynamic_vocab`, `articulation_vocab`, `ornament`, `non_chord_tone`, `tuning_system`, `world_tradition`, `historical_era`, `harmonic_function`, `max_melodic_leap`, `contrapuntal_motion`, `cadence`, `harmonic_progression`, `texture`, `form`, `instrument_range`, `parallel_fifths`, and `voice_crossing`.
- Explicit local overrides with audit traces.
- IR metadata for tempo, meter, key signature, track name, channel, and program.
- MIDI rendering with tempo, time signature, key signature, track name, program change, and per-track channel.
- MusicXML rendering for notation interchange.
- WAV audio rendering for direct audition.
- CLI commands for compile/check/export/diagnose/ast/ir/theory/repl.
- LSP server with publishDiagnostics, hover, completion, and go-to-definition.

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
```

A MusicLang project contains `music.toml`, `src/main.music`, and build outputs under `build/`.

## CLI quickstart

```bash
music check examples/minimal.music
music check examples/demo_jazz_complete.music --strict
music analyze examples/demo_jazz_complete.music --strict
music compile examples/demo_jazz_complete.music -o /tmp/musiclang-jazz.mid --strict
music export examples/minimal.music --format midi -o /tmp/musiclang-minimal.mid
music export examples/minimal.music --format musicxml -o /tmp/musiclang-minimal.musicxml
music export examples/minimal.music --format wav -o /tmp/musiclang-minimal.wav
music export examples/demo_jazz_complete.music --format wav -o /tmp/musiclang-jazz.wav --strict
music diagnose examples/style_violation.music --json
music ast examples/minimal.music
music ir examples/minimal.music
music styles
music theory --domain scales
music theory --domain dynamics
music theory --domain harmonic_functions
music theory --find maqam
```

`--strict` is the quality gate for publishable/listening material. It rejects every diagnostic, including warning-only style diagnostics, and `music analyze --strict` also rejects excessive repeated bars. CI runs strict analysis and strict MIDI/MusicXML/WAV export smoke tests for the complete Jazz demo.

Listening demos are expected to pass without diagnostic suppression: no `override` for cleanup, no `severity_*: off`, no warnings, and no uncontrolled repeated-bar padding.

Run the REPL:

```bash
music repl
```

Inside the REPL:

```text
:load examples/override.music
:diagnose
:show source
:show ir
:export /tmp/demo.mid
:reset
:quit
```

Run the LSP server over stdio:

```bash
cargo run -q -p musiclang-lsp
```

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

- `examples/minimal.music`
- `examples/loop.music`
- `examples/control_flow.music`
- `examples/override.music`
- `examples/style_violation.music`
- `examples/custom_style.music`
- `examples/custom_style_violation.music`

See `docs/requirements/musiclang.md` for the broader product requirements.
