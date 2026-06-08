# MusicLang Style System

A style declaration activates music-theory checks during compilation. The compiler validates explicit source; it does not compose or rewrite music.

## Built-in behavior

The built-in registry currently exposes `Classical`, `Modal`, `Jazz`, and `Minimalist`. Use `music styles` to list available style packages. `score demo style Jazz { ... }` can select a built-in style without a local declaration, while a local `style Jazz { ... }` can override it.

`style Classical` enables C-major scale membership, common triad vocabulary, default 4/4 meter, and a broad classical tempo range.

Custom styles can configure rule inputs and inherit from other styles:

```musiclang
style Chamber extends Classical {
  scale: C D E F G A B
  scale_pattern: C major
  mode_pattern: D dorian
  chord_vocab: C E G; F A C; G B D
  chord_quality_vocab: major minor dominant7
  set_class_vocab: 016 all_interval_tetrachord
  meter: 3/4
  meter_catalog: 3/4 6/8
  tempo_range: 60..132
  rhythm_vocab: 1/4 1/8 1/16
  rhythm_concept: ostinato
  dynamic_vocab: p mp mf f
  articulation_vocab: staccato tenuto accent
  ornament: trill mordent turn
  non_chord_tone: passing_tone neighbor_tone
  tuning_system: equal_temperament_12 just_intonation
  world_tradition: maqam hindustani_raga
  historical_era: baroque classical jazz
  harmonic_function: tonic predominant dominant
  max_melodic_leap: P5
  contrapuntal_motion: contrary oblique similar
  cadence: authentic
  harmonic_progression: tonic predominant dominant tonic
  texture: homophony
  form: ternary
  instrument_range: 40 C3 C7
}
```

## Enforced rule IDs

- `scale`: note pitch classes must belong to the active scale.
- `scale_pattern`: derives the active scale from `tonic scale_id` using the `scales` theory catalog, then enforces it through `scale`.
- `mode_pattern`: derives the active pitch collection from `tonic mode_id` using the `modes` theory catalog, then enforces it through `scale`.
- `chord_vocab`: chord pitch classes must match one configured vocabulary entry.
- `chord_quality_vocab`: chord pitch-class interval structures must match one configured quality from the `chord_qualities` theory catalog.
- `set_class_vocab`: chord pitch-class sets must match one configured entry from the `set_classes` theory catalog.
- `meter`: score `meter` metadata must match the active style meter.
- `meter_catalog`: score `meter` metadata must match one configured meter from the `meters` theory catalog.
- `tempo_range`: score `tempo` metadata must stay within the configured BPM range.
- `rhythm_vocab`: note and chord durations must belong to the configured rhythmic vocabulary.
- `rhythm_concept`: score rhythm pattern must satisfy configured concepts from the `rhythms` theory catalog (`ostinato` requires a repeating duration cell, `syncopation` requires an offbeat attack, `hemiola` requires a three-in-two duration pattern, and `swing` requires a long-short 2:1 duration pair).
- `dynamic_vocab`: `dynamic` statements must use configured entries from the `dynamics` theory catalog.
- `articulation_vocab`: `articulation` statements must use configured entries from the `ornaments` theory catalog.
- `ornament`: `ornament` annotation blocks must use configured entries from the `ornaments` theory catalog.
- `non_chord_tone`: `non_chord_tone` annotation blocks must use configured entries from the `non_chord_tones` theory catalog.
- `tuning_system`: `tuning_system` annotation blocks must use configured entries from the `tuning_systems` theory catalog.
- `world_tradition`: `world_tradition` annotation blocks must use configured entries from the `world_traditions` theory catalog.
- `historical_era`: `historical_era` annotation blocks must use configured entries from the `style_eras` theory catalog.
- `harmonic_function`: `harmonic_function` annotation blocks must use configured entries from the `harmonic_functions` theory catalog.
- `max_melodic_leap`: consecutive notes in a voice must not exceed the configured interval.
- `contrapuntal_motion`: simultaneous voice pairs must move only through allowed motion types (`parallel`, `similar`, `contrary`, `oblique`).
- `instrument_range`: notes in a voice with `program` must fit the configured MIDI program range.
- `parallel_fifths`: simultaneous voice pairs must not move through consecutive perfect fifths.
- `voice_crossing`: an upper voice must not cross below a lower voice at the same tick.
- `cadence`: final sonorities must satisfy configured cadence patterns (`authentic`, `plagal`, `deceptive`, `half`).
- `harmonic_progression`: sonorities must contain the configured functional progression (`tonic`, `predominant`, `dominant`, `secondary_dominant`, `submediant`).
- `texture`: compiled tracks must satisfy configured texture (`monophony`, `polyphony`, `homophony`, `heterophony`).
- `form`: explicit `section` labels must match the configured entry from the `forms` theory catalog, such as `binary`, `ternary`, `sonata`, or `rondo`.

Unknown override rules fail with `ML_STYLE_UNKNOWN_RULE`.

## Theory knowledge base

The enforced rules are backed by `musiclang-core::theory_catalog()`, a first-class queryable theory knowledge base exposed through the core API and `music theory` CLI command. It covers intervals, scales, modes, chord qualities, cadences, meters, rhythm concepts, dynamics, forms, textures, ornaments, contrapuntal motion, non-chord tones, harmonic functions, post-tonal set classes, tuning systems, world-tradition modal/rhythmic systems, and historical/style eras.

Every theory catalog domain is also a valid style key. The compiler validates referenced theory IDs during style loading and emits `ML_STYLE_UNKNOWN_THEORY_ENTRY` for unknown entries or `ML_STYLE_UNKNOWN_KEY` for unknown style keys.

```musiclang
style TheoryRich {
  scales: blues major_pentatonic
  harmonic_functions: tonic dominant secondary_dominant
  world_traditions: maqam hindustani_raga
  set_classes: 016 all_interval_tetrachord
}
```

Styles can also define custom theory domains with `theory_<domain>: ...` and then validate references to that custom domain in the same style block.

```musiclang
style MicrotonalPractice {
  theory_microgestures: bend flutter split_tone
  microgestures: bend split_tone
}
```

Styles can configure rule severity with `severity_<rule>: error`, `severity_<rule>: warning`, or `severity_<rule>: off`. `error` is the default and blocks compilation; `warning` records a diagnostic without blocking output; `off` disables that rule.

Styles can declare custom rule IDs with `rule_<id>: description`. These rule IDs participate in override validation, so local exceptions can be audited without extending the compiler's built-in rule enum.

```musiclang
style Experimental {
  rule_microtonal_collision: locally defined microtonal voice interaction
}
score demo {
  voice lead {
    override microtonal_collision allow reason "intentional beating" {
      note C4, 1/4
    }
  }
}
```

## Local style scopes

A score can select a declared style with `score name style StyleName { ... }`. Sections can also switch style locally:

```musiclang
style Classical
style Sparse {
  scale: C E G
}
score demo style Classical {
  voice lead {
    with style Sparse {
      note E4, 1/4
    }
  }
}
```

## Override policy

Overrides must be local and auditable:

```musiclang
override scale allow reason "intentional chromatic color" {
  note F#4, 1/4
}
```

Overrides suppress style rules only. They do not bypass parser, type, name-resolution, or core music errors.

## Diagnostic codes

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
