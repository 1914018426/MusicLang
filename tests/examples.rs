use std::collections::BTreeMap;
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
        "examples/drum_groove.music",
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
fn listening_demos_do_not_bypass_rules() {
    for path in [
        "examples/demo_classical_minuet.music",
        "examples/demo_jazz_blues.music",
        "examples/demo_jazz_complete.music",
        "examples/demo_minimal_pulse.music",
        "examples/demo_cinematic_ambient.music",
        "examples/drum_groove.music",
    ] {
        let source = fs::read_to_string(path).unwrap();

        assert!(!source.contains("override "), "{path} uses override");
        assert!(!source.contains(": off"), "{path} disables a rule");
    }
}

#[test]
fn listening_demos_keep_repetition_under_control() {
    for path in [
        "examples/demo_classical_minuet.music",
        "examples/demo_jazz_blues.music",
        "examples/demo_jazz_complete.music",
        "examples/demo_minimal_pulse.music",
        "examples/demo_cinematic_ambient.music",
        "examples/drum_groove.music",
    ] {
        let ir = compile_example(path);
        let analysis = repeated_bars(&ir);

        assert!(
            analysis.ratio_percent <= 50,
            "{path} repeats {}% of its bars",
            analysis.ratio_percent
        );
        assert!(
            analysis.longest_run <= 4,
            "{path} has {} identical bars in a row",
            analysis.longest_run
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RepeatedBarAnalysis {
    ratio_percent: u32,
    longest_run: u32,
}

fn repeated_bars(ir: &musiclang_core::ScoreIr) -> RepeatedBarAnalysis {
    let meter = ir.meter.unwrap_or_default();
    let bar_ticks = (ir.ticks_per_quarter * u32::from(meter.numerator) * 4
        / u32::from(meter.denominator))
    .max(1);
    let duration_ticks = ir
        .tracks
        .iter()
        .flat_map(|track| track.events.iter())
        .map(|event| event.start_tick + event.duration_ticks)
        .max()
        .unwrap_or(0);
    let duration_bars = duration_ticks.div_ceil(bar_ticks).max(1);
    let mut signatures = Vec::new();
    for bar in 0..duration_bars {
        let bar_start = bar * bar_ticks;
        let bar_end = bar_start + bar_ticks;
        let mut entries = Vec::new();
        for track in &ir.tracks {
            for event in &track.events {
                if event.start_tick >= bar_start && event.start_tick < bar_end {
                    entries.push(format!(
                        "{}:{}:{}:{}",
                        track.name,
                        event.start_tick - bar_start,
                        event.duration_ticks,
                        event.pitch.midi_number().unwrap_or(0)
                    ));
                }
            }
        }
        entries.sort();
        signatures.push(entries.join("|"));
    }
    let repeated_count = signatures
        .iter()
        .fold(BTreeMap::<&String, u32>::new(), |mut counts, signature| {
            *counts.entry(signature).or_default() += 1;
            counts
        })
        .values()
        .map(|count| count.saturating_sub(1))
        .sum::<u32>();
    let mut longest_run = 0;
    let mut current_run = 0;
    let mut previous = None;
    for signature in &signatures {
        if Some(signature) == previous {
            current_run += 1;
        } else {
            current_run = 1;
            previous = Some(signature);
        }
        longest_run = longest_run.max(current_run);
    }

    RepeatedBarAnalysis {
        ratio_percent: repeated_count * 100 / duration_bars,
        longest_run,
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
fn voice_mix_metadata_and_drums_lower_to_tracks() {
    let ir = musiclang_compiler::compile_source(
        r#"
        score mix {
          voice lead {
            instrument sax
            channel 2
            volume 92
            pan 36
            note C4, 1/4
          }
          voice kit {
            instrument drums
            channel 9
            drum kick, 1/8
            drum snare, 1/8
          }
        }
        "#,
    )
    .unwrap();

    assert_eq!(ir.tracks[0].program, Some(65));
    assert_eq!(ir.tracks[0].channel, 2);
    assert_eq!(ir.tracks[0].volume, Some(92));
    assert_eq!(ir.tracks[0].pan, Some(36));
    assert_eq!(ir.tracks[1].program, Some(0));
    assert_eq!(ir.tracks[1].channel, 9);
    assert_eq!(ir.tracks[1].events[0].pitch.midi_number().unwrap(), 36);
    assert_eq!(ir.tracks[1].events[1].pitch.midi_number().unwrap(), 38);
}
