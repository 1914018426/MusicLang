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
    instrument sax
    channel 2
    volume 96
    pan 64
    call motif
  }
  voice drums {
    instrument drums
    channel 9
    drum kick, 1/4
    drum snare, 1/4
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
- `program midi_program`, `instrument midi_program`, or `instrument name` for built-ins such as `piano`, `bass`, `sax`, `trumpet`, `violin`, `strings`, `synth_pad`, and `drums`
- `channel 0..15` for explicit MIDI channel selection; General MIDI drums conventionally use channel `9`
- `volume 0..127` for MIDI channel volume (CC7)
- `pan 0..127` for MIDI pan position (CC10)
- `drum name, duration_expr` for General MIDI drum hits such as `kick`, `snare`, `closed_hat`, `open_hat`, `ride`, and `crash`
- `rest duration_expr`
- `glissando start_pitch to end_pitch steps count, duration_expr`
- `tremolo first_pitch with second_pitch repeats count, duration_expr`
- `degree scale_degree octave, duration_expr`
- `scale tonic mode octave, duration_expr`
- `pedal pitch, duration_expr repeats count`
- `ostinato count { ... }`, `sequence count by interval { ... }`, `tuplet count in duration_expr { ... }`, and `transpose interval { ... }`
- `arpeggio [pitch_expr, ...], duration_expr`, `arpeggio root quality, duration_expr`, and optional `inv index` for named arpeggios
- `strum [pitch_expr, ...], duration_expr by delay_expr`, `strum root quality, duration_expr by delay_expr`, and optional `inv index` for named strums
- `roman numeral, duration_expr`, `progression numeral ... , duration_expr`, `cadence kind, duration_expr`, and `modulate tonic major|minor`
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

Supported expression values are integers, booleans, pitches, intervals, durations, strings, lists, tuples, and dict event values.

```musiclang
note C4 + M3, 1/4
note E4 - m3, 1/8
let d = 1/4
let legacy = duration 1/8
if i == 1 { note G4, d }
```

Algorithmic material can be expressed as values and lowered with `play`:

```musiclang
fn seed() = [{p:at([C4, D4, E4, G4], i), d:1/8, skip:i == 2} for i in 0..4]
fn shape(events) = [event.with({d:1/4}) for event in events if not event.skip]
score generated {
  voice lead {
    play shape(seed())
  }
}
```

Expression builtins:

```musiclang
fn embellish(event) = event.with({d:duration("1/16")})
fn keep(event) = event.p != pitch("C4")
let phrase = concat(repeat({p:pitch("C4"), d:1/8}, 2), [{p:D4, d:1/8}])
play stretch(map(filter(phrase, "keep"), "embellish"), 2)
```

- `at(collection, index)` returns a list or tuple item at a zero-based index.
- `len(collection)` returns the item count of a list or tuple.
- `with(dict, patch)` returns a dict with patch fields merged over the original.
- `merge(dict, patch)` is equivalent to `with`.
- `cat(values...)` and `concat(values...)` concatenate values into one list.
- `map(collection, function_name)` maps a function over each collection item; `function_name` may be a bare function identifier or string.
- `filter(collection, function_name)` keeps collection items for which the function returns `true`; `function_name` may be a bare function identifier or string.
- `mapi(collection, function_name)` maps a function over `(index, item)` pairs; `function_name` may be a bare function identifier or string.
- Transform builtins may also be chained as methods, such as `phrase.mapi(mark).filter(keep).map(lift)`.
- `repeat(value, count)` repeats a value into a list.
- `stretch(collection, factor)` multiplies event durations by an integer factor.
- `duration("1/8")` parses a duration string.
- `pitch("C4")` parses a pitch string.
- `first(collection)` returns the first item in a non-empty list or tuple.
- `not bool_expr` negates a boolean expression.

List comprehensions use `[item for name in source if condition]`. The `if` clause is optional, `source` must evaluate to a list or tuple collection, and the condition must evaluate to a boolean. Range expressions use half-open integer ranges such as `0..4`; descending ranges such as `3..0` produce `3, 2, 1`.

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
  phrase_concept: periodic_phrase motivic_development
  dynamic_vocab: p mp mf f
  articulation_vocab: staccato tenuto accent
  ornament: trill mordent turn
  non_chord_tone: passing_tone neighbor_tone
  tuning_system: equal_temperament_12 just_intonation
  world_tradition: maqam hindustani_raga
  historical_era: baroque classical jazz
  harmonic_function: tonic predominant dominant secondary_dominant submediant
  max_melodic_leap: P5
  voice_spacing: P8
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

The built-in `Jazz` style combines chromatic pitch allowance with quality gates for `swing`, `syncopation`, `blues_inflection`, `call_response`, `walking_or_riff_bass`, predominant-dominant-tonic functional motion, and authentic cadence. Normal compilation reports missing identity as warnings, while strict commands reject those warnings. Harmonic/counterpoint rules operate on pitched voices and ignore General MIDI channel 9 drum tracks.

## CLI

```bash
music new demo_song
music build
music build --manifest path/to/music.toml
music build --strict
music check input.music
music check input.music --strict
music analyze input.music --strict
music analyze input.music --json
music compile input.music -o output.mid --strict
music export input.music --format midi -o output.mid
music export input.music --format musicxml -o output.musicxml
music export input.music --format wav -o output.wav
music export input.music --format wav -o output.wav --strict
music diagnose input.music --json
music ast input.music
music ir input.music
music styles
music styles --json
music formats
music formats --json
music idioms
music idioms --json
music theory
music theory --json
music theory --domain dynamics
music theory --domain harmonic_functions
music theory --domain dynamics --json
music theory --find maqam
music theory --find maqam --json
music repl
```

REPL commands include `:load path.music`, `:style StyleName`, `:diagnose`, `:styles`, `:formats`, `:idioms`, `:theory [domain|entry]`, `:play`, `:stop`, `:export path.mid`, `:show source`, `:show ir`, `:reset`, and `:quit`. `:play` renders the current buffer to a temporary MIDI file and starts `MUSICLANG_PLAYER` asynchronously when configured; without that environment variable it prints the rendered file path.

`check --strict`, `compile --strict`, `build --strict`, and `export --strict` reject any compiler diagnostic before accepting or writing output. Strict quality gates also reject explicit suppression through `override` blocks or `severity_*: off`. `analyze --strict` applies the same zero-diagnostic requirement and also enforces listening-quality repetition thresholds. Project manifests support `name`, `source`, `output`, `format`, and `strict` keys with `#` comments; `strict` must be `true` or `false`. Set `strict = true` to make `music build` use strict output rules by default.

## Diagnostics

`severity_<rule>: warning` diagnostics are non-blocking for non-strict compilation and export, but still reported by `diagnose`, REPL diagnostics, and LSP publishDiagnostics. Strict commands treat warnings as failed quality gates.

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
- `ML_STYLE_MELODIC_CONCEPT`
- `ML_STYLE_ENSEMBLE_CONCEPT`
- `ML_STYLE_BASS_CONCEPT`
- `ML_STYLE_DYNAMIC_VOCAB`
- `ML_STYLE_ARTICULATION_VOCAB`
- `ML_STYLE_ORNAMENT`
- `ML_STYLE_NON_CHORD_TONE`
- `ML_STYLE_TUNING_SYSTEM`
- `ML_STYLE_WORLD_TRADITION`
- `ML_STYLE_HISTORICAL_ERA`
- `ML_STYLE_HARMONIC_FUNCTION`
- `ML_STYLE_MAX_MELODIC_LEAP`
- `ML_STYLE_VOICE_SPACING`
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
- `ML_STYLE_UNKNOWN_NAME`
- `ML_STYLE_UNKNOWN_THEORY_ENTRY`
- `ML_STYLE_UNKNOWN_IDIOM_ENTRY`
- `ML_STYLE_INHERITANCE_CYCLE`

## LSP

`musiclang-lsp` runs over stdio and supports:

- `textDocument/publishDiagnostics` backed by `musiclang_compiler::diagnose_source`
- `textDocument/hover` for language keywords, style rules, local symbols, and expression builtins
- `textDocument/completion` for language keywords, rule IDs, built-in styles, local symbols, theory entries, expression builtins, and method-style expression builtins
- `textDocument/signatureHelp` for note/chord statements, expression builtins, method-style expression builtins, and local function calls
- `textDocument/semanticTokens/full` for keywords, functions, variables, styles, numbers, strings, comments, and operators
- `textDocument/definition` for `fn`, `let`, and `style` symbols
- `textDocument/references` for word-level references in the current document
- `textDocument/documentHighlight` for word-level highlights in the current document
- `textDocument/documentSymbol` for major declarations and blocks
- `textDocument/inlayHint` for note and chord argument labels
- `textDocument/formatting` for deterministic MusicLang source formatting
- `textDocument/foldingRange` for brace-delimited blocks
- `textDocument/prepareRename` and `textDocument/rename` for local word-symbol renames
- `textDocument/selectionRange` for expanding word selections to line and block ranges
- `workspace/symbol` for searching open documents
- `textDocument/codeAction` quick fixes surfaced from compiler diagnostic help

IDE-facing behavior is covered by unit tests for word lookup, completion entries, signature help, semantic tokens, function/style/variable definition lookup, hover content, diagnostic conversion, document symbols, references, document highlights, inlay hints, formatting, folding ranges, rename, selection ranges, workspace symbols, and code actions.
