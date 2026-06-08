# MusicLang Examples

Create and build a standalone MusicLang project with:

```bash
music new demo_song
cd demo_song
music build
```

Run the bundled examples with:

```bash
music check examples/minimal.music
music export examples/minimal.music -o /tmp/minimal.mid
music diagnose examples/style_violation.music
```

## Minimal score

```musiclang
score demo {
  voice lead {
    note C4, 1/4
    chord [C4, E4, G4], 1/2
  }
}
```

## Control flow

```musiclang
fn motif {
  note C4, 1/8
}
score demo {
  voice lead {
    let d = 1/4
    for i in 0..3 {
      if i == 1 { call motif }
      note E4, d
    }
  }
}
```

## Pitch expressions

```musiclang
score demo {
  voice lead {
    note C4 + M3, 1/4
    note E4 - m3, 1/4
  }
}
```

## Algorithmic expression pipeline

`examples/algorithmic_expression.music` demonstrates value-level phrase generation, filtering, event patching, metadata, and MIDI lowering without using override suppression:

```musiclang
fn seed() = [{p:at([C4, D4, E4, G4, A4], i), d:1/8, skip:i == 2} for i in 0..5]
fn mirror() = [{p:at([A4, G4, E4, D4, C4], i), d:1/8, skip:false} for i in 0..5]
fn shape(events) = [event.with({d:1/4}) for event in events if not event.skip]

score algorithmic_expression {
  title "Algorithmic Expression Study"
  composer "MusicLang"
  tempo 104
  meter 4/4
  key C major

  voice lead {
    instrument piano
    channel 1
    volume 92
    pan 64
    play shape(seed())
    play shape(mirror())
  }
}
```

Run it through the same gates as hand-written material:

```bash
music check examples/algorithmic_expression.music --strict
music export examples/algorithmic_expression.music --format midi -o /tmp/algorithmic-expression.mid --strict
music diagnose examples/algorithmic_expression.music --json
```
