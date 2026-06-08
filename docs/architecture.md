# MusicLang Architecture

MusicLang is organized as a Rust workspace with small crates around a compiler pipeline.

```text
source
  -> musiclang-parser: lexer, parser, AST, spans
  -> musiclang-compiler: expression evaluation, style checks, IR lowering
  -> musiclang-midi: MIDI rendering
  -> musiclang-render: MusicXML and WAV rendering
  -> musiclang-cli: command line and REPL
  -> musiclang-lsp: editor diagnostics, hover, completion, definition
```

## Crates

- `musiclang-core`: shared music types, diagnostics, spans, style context, IR
- `musiclang-parser`: tokenizes and parses `.music` source into AST
- `musiclang-compiler`: resolves names, evaluates expressions, checks style rules, lowers to IR
- `musiclang-midi`: renders IR into Standard MIDI bytes using `midly`
- `musiclang-render`: renders IR into MusicXML and WAV audio
- `musiclang-cli`: exposes `compile`, `check`, `export`, `diagnose`, `ast`, `ir`, and `repl`
- `musiclang-lsp`: exposes an LSP stdio server with diagnostics, hover, completion, and go-to-definition

## Compatibility contract

The examples in `examples/*.music` and integration tests in `tests/examples.rs` are the current compatibility contract. Public facades are kept stable:

- `musiclang_parser::parse_source`
- `musiclang_compiler::compile_source`
- `musiclang_compiler::diagnose_source`
- `musiclang_midi::render_midi`
