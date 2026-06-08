use std::fs;

fn compile_example(path: &str) -> musiclang_core::ScoreIr {
    let source = fs::read_to_string(path).unwrap();
    musiclang_compiler::compile_source(&source).unwrap()
}

#[test]
fn valid_examples_compile_to_midi() {
    for path in [
        "examples/minimal.music",
        "examples/loop.music",
        "examples/control_flow.music",
        "examples/override.music",
        "examples/custom_style.music",
        "examples/demo_classical_minuet.music",
        "examples/demo_jazz_blues.music",
        "examples/demo_jazz_complete.music",
        "examples/demo_minimal_pulse.music",
        "examples/demo_cinematic_ambient.music",
    ] {
        let ir = compile_example(path);
        let midi = musiclang_midi::render_midi(&ir).unwrap();

        assert!(
            midi.starts_with(b"MThd"),
            "{path} did not render MIDI header"
        );
    }
}

#[test]
fn style_violation_examples_keep_stable_diagnostic_code() {
    for path in [
        "examples/style_violation.music",
        "examples/custom_style_violation.music",
    ] {
        let source = fs::read_to_string(path).unwrap();
        let diagnostics = musiclang_compiler::compile_source(&source).unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_SCALE", "{path}");
        assert_eq!(diagnostics[0].rule.as_deref(), Some("scale"), "{path}");
    }
}

#[test]
fn override_example_keeps_audit_trace() {
    let ir = compile_example("examples/override.music");

    assert_eq!(ir.overrides.len(), 1);
    assert_eq!(ir.overrides[0].rule, "scale");
    assert_eq!(
        ir.overrides[0].reason.as_deref(),
        Some("intentional chromatic color")
    );
}

#[test]
fn voice_volume_and_pan_lower_to_track_metadata() {
    let ir = musiclang_compiler::compile_source(
        r#"
        score mix {
          voice lead {
            program 65
            volume 92
            pan 36
            note C4, 1/4
          }
        }
        "#,
    )
    .unwrap();

    assert_eq!(ir.tracks[0].program, Some(65));
    assert_eq!(ir.tracks[0].volume, Some(92));
    assert_eq!(ir.tracks[0].pan, Some(36));
}
