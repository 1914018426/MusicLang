# MusicLang Language Reference

MusicLang is a compiler-first language for writing music as source code. It targets MIDI, MusicXML, and WAV output, and exposes diagnostics through CLI, REPL, and LSP.

## Program structure

A file may declare one or more optional named styles, zero or more functions, and one score. A score can select one of the declared styles with `score name style StyleName`:

```musiclang
style Classical
fn motif {
  note C4, 1/8
}
score demo style Classical {
  tempo 96
  meter 4/4
  voice lead {
    program 40
    volume 96
    pan 64
    call motif
  }
}
```

## Statements

- `style Name`, `style Name { key: value }`, or `style Child extends Parent { key: value }`
- `score name { ... }` or `score name style StyleName { ... }`
- `tempo bpm`
- `meter numerator/denominator`
- `key tonic major|minor`
- `voice name { ... }`
- `program midi_program` or `instrument midi_program`
- `volume 0..127` for MIDI channel volume (CC7)
- `pan 0..127` for MIDI pan position (CC10)
- `dynamic mark` for catalog dynamics such as `p`, `mf`, `f`, or `sfz`
- `velocity 0..127` for explicit MIDI velocity
- `articulation mark` for catalog ornaments/articulations such as `staccato`, `tenuto`, `accent`, or `legato`
- `section label { ... }` for explicit form markers such as `A`, `B`, `exposition`, or `recapitulation`
- `ornament kind { ... }` for explicit ornament annotations such as `trill`, `mordent`, or `turn`
- `non_chord_tone kind { ... }` for explicit non-chord tone annotations such as `passing_tone` or `neighbor_tone`
- `tuning_system kind { ... }` for explicit tuning-system annotations such as `equal_temperament_12` or `just_intonation`
- `world_tradition kind { ... }` for explicit world-tradition annotations such as `maqam` or `hindustani_raga`
- `historical_era kind { ... }` for explicit style-era annotations such as `baroque`, `classical`, or `jazz`
- `harmonic_function kind { ... }` for explicit harmonic-function annotations such as `tonic`, `predominant`, or `dominant`
- `note pitch_expr, duration_expr`
- `chord [pitch_expr, ...], duration_expr`
- `let name = expr`
- `for i in 0..4 { ... }`
- `if expr == expr { ... }`
- `fn name { ... }` and `call name`
- `override rule allow reason "text" { ... }`
- `with style StyleName { ... }`

## Expressions

Supported expression values are integers, booleans, pitches, intervals, durations, and strings.

```musiclang
note C4 + M3, 1/4
note E4 - m3, 1/8
let d = duration 1/4
if i == 1 { note G4, d }
```

## Style configuration

```musiclang
style Sparse {
  scale: C E G
  scale_pattern: C major_pentatonic
  mode_pattern: D dorian
  chord_vocab: C E G
  chord_quality_vocab: major minor dominant7
  set_class_vocab: 016 all_interval_tetrachord
  meter: 3/4
  meter_catalog: 3/4 6/8
  tempo_range: 60..120
  rhythm_vocab: 1/4 1/8 1/16
  rhythm_concept: ostinato syncopation hemiola swing
  dynamic_vocab: p mp mf f
  articulation_vocab: staccato tenuto accent
  ornament: trill mordent turn
  non_chord_tone: passing_tone neighbor_tone
  tuning_system: equal_temperament_12 just_intonation
  world_tradition: maqam hindustani_raga
  historical_era: baroque classical jazz
  harmonic_function: tonic predominant dominant secondary_dominant submediant
  max_melodic_leap: P5
  contrapuntal_motion: contrary oblique similar
  cadence: authentic plagal deceptive half
  harmonic_progression: tonic predominant dominant tonic
  texture: homophony
  form: ternary
  instrument_range: 40 C3 C7
  severity_scale: warning
}
```

## Theory catalog

MusicLang includes a broad theory catalog used by style declarations and the `music theory` CLI. It covers intervals, scales, modes, chord qualities, cadences, meters, rhythm concepts, dynamics, forms, textures, ornaments, contrapuntal motion types, non-chord tones, harmonic functions, post-tonal set classes, tuning systems, world-tradition modal/rhythmic systems, and historical/style eras. Any catalog domain can be used as a style key, and the compiler validates referenced theory IDs. Styles can add project-local theory domains with `theory_<domain>: entry_a entry_b`, then reference those custom entries with `<domain>: entry_a` in the same style block. Styles can also declare custom rule IDs with `rule_<id>: description`; those IDs are accepted by `override <id> allow` and recorded in the audit trace.

## CLI

```bash
music new demo_song
music build
music check input.music
music export input.music --format midi -o output.mid
music export input.music --format musicxml -o output.musicxml
music export input.music --format wav -o output.wav
music diagnose input.music --json
music ast input.music
music ir input.music
music styles
music theory
music theory --domain dynamics
music theory --domain harmonic_functions
music theory --find maqam
music repl
```

## Diagnostics

`severity_<rule>: warning` diagnostics are non-blocking but still reported by `diagnose`, REPL diagnostics, and LSP publishDiagnostics.

Stable diagnostic codes include:

- `ML_PARSE_*` for parser errors
- `ML_CORE_PITCH`, `ML_CORE_DURATION`, `ML_CORE_CHORD`
- `ML_RESOLVE_UNKNOWN_NAME`
- `ML_RESOLVE_DUPLICATE_NAME`
- `ML_RESOLVE_RECURSIVE_CALL`
- `ML_TYPE_MISMATCH`
- `ML_EVAL_UNSUPPORTED_OP`
- `ML_STYLE_SCALE`
- `ML_STYLE_CHORD_VOCAB`
- `ML_STYLE_CHORD_QUALITY_VOCAB`
- `ML_STYLE_SET_CLASS_VOCAB`
- `ML_STYLE_METER`
- `ML_STYLE_METER_CATALOG`
- `ML_STYLE_TEMPO_RANGE`
- `ML_STYLE_RHYTHM_VOCAB`
- `ML_STYLE_RHYTHM_CONCEPT`
- `ML_STYLE_DYNAMIC_VOCAB`
- `ML_STYLE_ARTICULATION_VOCAB`
- `ML_STYLE_ORNAMENT`
- `ML_STYLE_NON_CHORD_TONE`
- `ML_STYLE_TUNING_SYSTEM`
- `ML_STYLE_WORLD_TRADITION`
- `ML_STYLE_HISTORICAL_ERA`
- `ML_STYLE_HARMONIC_FUNCTION`
- `ML_STYLE_MAX_MELODIC_LEAP`
- `ML_STYLE_CONTRAPUNTAL_MOTION`
- `ML_STYLE_INSTRUMENT_RANGE`
- `ML_STYLE_PARALLEL_FIFTHS`
- `ML_STYLE_VOICE_CROSSING`
- `ML_STYLE_CADENCE`
- `ML_STYLE_HARMONIC_PROGRESSION`
- `ML_STYLE_TEXTURE`
- `ML_STYLE_FORM`
- `ML_STYLE_UNKNOWN_RULE`
- `ML_STYLE_UNKNOWN_KEY`
- `ML_STYLE_UNKNOWN_THEORY_ENTRY`

## LSP

`musiclang-lsp` runs over stdio and supports:

- `textDocument/publishDiagnostics` backed by `musiclang_compiler::diagnose_source`
- hover for language keywords and style rules
- completion for language keywords and rule IDs
- go-to-definition for `fn` and `let` symbols

IDE-facing behavior is covered by unit tests for word lookup, completion entries, function definition lookup, variable definition lookup, hover content, and diagnostic conversion.
