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
    let d = duration 1/4
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
