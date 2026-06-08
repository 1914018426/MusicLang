# MusicLang Architecture

MusicLang is organized as a Rust workspace with small crates around a compiler pipeline.

```text
source
  -> musiclang-parser: lexer, parser, expression AST, spans
  -> musiclang-compiler: name checks, expression evaluation, style checks, IR lowering
  -> musiclang-midi: MIDI rendering
  -> musiclang-render: MusicXML and WAV rendering
  -> musiclang-cli: command line, strict gates, project build, REPL
  -> musiclang-lsp: editor diagnostics, semantic tokens, hover, completion, signature help, navigation, refactors
```

## Crates

- `musiclang-core`: shared music types, diagnostics, spans, style context, IR
- `musiclang-parser`: tokenizes and parses `.music` source into AST
- `musiclang-compiler`: resolves names, evaluates expressions, checks style rules, lowers to IR
- `musiclang-midi`: renders IR into Standard MIDI bytes using `midly`
- `musiclang-render`: renders IR into MusicXML and WAV audio
- `musiclang-cli`: exposes `compile`, `check`, `export`, `diagnose`, `ast`, `ir`, `analyze`, `styles`, `idioms`, `theory`, and `repl`
- `musiclang-lsp`: exposes an LSP stdio server with diagnostics, hover, completion, method-aware expression completions, signature help, semantic tokens, definition, references, document highlights, document symbols, inlay hints, formatting, folding ranges, rename, selection ranges, workspace symbols, and diagnostic quick-fix code actions

## Compatibility contract

The examples in `examples/*.music` and integration tests in `tests/examples.rs` are the current compatibility contract. `examples/algorithmic_expression.music` specifically locks value-level phrase generation across parser, compiler, CLI export, MIDI rendering, and LSP discovery surfaces. Public facades are kept stable:

- `musiclang_parser::parse_source`
- `musiclang_parser::parse_source_file`
- `musiclang_compiler::compile_source`
- `musiclang_compiler::compile_source_file`
- `musiclang_compiler::diagnose_source`
- `musiclang_compiler::diagnose_source_file`
- `musiclang_midi::render_midi`
