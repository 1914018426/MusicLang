use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

fn run_music(args: &[&str]) -> std::process::Output {
    run_music_in(args, env!("CARGO_MANIFEST_DIR"))
}

fn run_music_in(args: &[&str], current_dir: &str) -> std::process::Output {
    music_command(args, current_dir).output().unwrap()
}

fn run_music_with_stdin(args: &[&str], input: &str) -> std::process::Output {
    let mut child = music_command(args, env!("CARGO_MANIFEST_DIR"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

fn music_command(args: &[&str], current_dir: &str) -> Command {
    let manifest = format!("{}/Cargo.toml", env!("CARGO_MANIFEST_DIR"));
    let mut command = Command::new(env!("CARGO"));
    command.current_dir(current_dir);
    command.args([
        "run",
        "-q",
        "--manifest-path",
        &manifest,
        "-p",
        "musiclang-cli",
        "--bin",
        "music",
        "--",
    ]);
    command.args(args);
    command
}

#[test]
fn music_version_reports_workspace_version() {
    let output = run_music(&["--version"]);

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "music 0.1.0"
    );
}

#[test]
fn music_check_accepts_example() {
    let output = run_music(&["check", "examples/minimal.music"]);

    assert!(output.status.success());
}

#[test]
fn music_check_strict_rejects_warning_only_diagnostics() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/check-strict-warning.music");
    fs::write(
        &input_path,
        r#"
style WarningScale {
  scale: C major
  severity_scale: warning
}

score warning_only style WarningScale {
  key C major
  voice lead {
    note C4, 1/4
    note F#4, 1/4
  }
}
"#,
    )
    .unwrap();

    assert!(run_music(&["check", &input_path]).status.success());
    let strict_output = run_music(&["check", &input_path, "--strict"]);

    assert!(!strict_output.status.success());
    let stderr = String::from_utf8_lossy(&strict_output.stderr);
    assert!(stderr.contains("ML_STYLE_SCALE"));
    assert!(stderr.contains("warning"));
}

#[test]
fn music_styles_lists_builtin_registry() {
    let output = run_music(&["styles"]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Classical"));
    assert!(stdout.contains("Jazz"));
    assert!(stdout.contains("Minimalist"));
}

#[test]
fn music_theory_lists_scales_domain() {
    let output = run_music(&["theory", "--domain", "scales"]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("scales:blues"));
    assert!(stdout.contains("major pentatonic"));
}

#[test]
fn music_theory_lists_dynamics_domain() {
    let output = run_music(&["theory", "--domain", "dynamics"]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("dynamics:mf"));
    assert!(stdout.contains("mezzo forte"));
}

#[test]
fn music_theory_lists_ornaments_domain() {
    let output = run_music(&["theory", "--domain", "ornaments"]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ornaments:staccato"));
    assert!(stdout.contains("short detached articulation"));
}

#[test]
fn music_new_and_build_create_project_output() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let sandbox = format!("{workspace}/target/music-cli-test-project");
    let project = format!("{sandbox}/demo_song");
    let _ = fs::remove_dir_all(&sandbox);
    fs::create_dir_all(&sandbox).unwrap();

    let new_output = run_music_in(&["new", "demo_song"], &sandbox);
    assert!(new_output.status.success());
    assert!(fs::metadata(format!("{project}/music.toml"))
        .unwrap()
        .is_file());
    let source = fs::read_to_string(format!("{project}/src/main.music")).unwrap();
    assert!(source.contains("instrument violin"));
    assert!(source.contains("channel 0"));
    assert!(source.contains("volume 96"));
    assert!(source.contains("pan 64"));
    assert!(source.contains("instrument drums"));
    assert!(source.contains("channel 9"));
    assert!(source.contains("drum kick"));

    let build_output = run_music_in(&["build"], &project);
    assert!(build_output.status.success());
    assert!(fs::read(format!("{project}/build/demo_song.mid"))
        .unwrap()
        .starts_with(b"MThd"));
}

#[test]
fn music_build_strict_rejects_warning_only_diagnostics() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let project = format!("{workspace}/target/music-cli-strict-build-project");
    let _ = fs::remove_dir_all(&project);
    fs::create_dir_all(format!("{project}/src")).unwrap();
    fs::write(
        format!("{project}/music.toml"),
        "name = \"strict-build\"\nsource = \"src/main.music\"\noutput = \"build/out.mid\"\nformat = \"midi\"\n",
    )
    .unwrap();
    fs::write(
        format!("{project}/src/main.music"),
        r#"
style WarningScale {
  scale: C major
  severity_scale: warning
}

score warning_only style WarningScale {
  key C major
  voice lead {
    note C4, 1/4
    note F#4, 1/4
  }
}
"#,
    )
    .unwrap();

    let build_output = run_music_in(&["build"], &project);
    assert!(build_output.status.success());
    assert!(fs::read(format!("{project}/build/out.mid"))
        .unwrap()
        .starts_with(b"MThd"));
    fs::remove_file(format!("{project}/build/out.mid")).unwrap();

    let strict_output = run_music_in(&["build", "--strict"], &project);

    assert!(!strict_output.status.success());
    assert!(!std::path::Path::new(&format!("{project}/build/out.mid")).exists());
    let stderr = String::from_utf8_lossy(&strict_output.stderr);
    assert!(stderr.contains("ML_STYLE_SCALE"));
    assert!(stderr.contains("warning"));
}

#[test]
fn music_diagnose_detects_style_violation() {
    let output = run_music(&["diagnose", "examples/style_violation.music"]);

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("ML_STYLE_SCALE"));
    assert!(stderr.contains("pitch"));
}

#[test]
fn music_diagnose_json_machine_readable() {
    let output = run_music(&["diagnose", "examples/style_violation.music", "--json"]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"code\":\"ML_STYLE_SCALE\""));
    assert!(stdout.contains("\"severity\""));
    assert!(stdout.contains("\"line\""));
    assert!(stdout.contains("\"span\":{"));
    assert!(stdout.contains("\"start\":"));
    assert!(stdout.contains("\"end\":"));
    assert!(stdout.contains("\"labels\":[]"));
    assert!(stdout.contains("\"related\":[]"));
}

#[test]
fn music_export_strict_rejects_warning_only_diagnostics() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/export-strict-warning.music");
    let output_path = format!("{workspace}/target/export-strict-warning.mid");
    let _ = fs::remove_file(&output_path);
    fs::write(
        &input_path,
        r#"
style WarningScale {
  scale: C major
  severity_scale: warning
}

score warning_only style WarningScale {
  key C major
  voice lead {
    note C4, 1/4
    note F#4, 1/4
  }
}
"#,
    )
    .unwrap();

    let output = run_music(&[
        "export",
        &input_path,
        "--format",
        "midi",
        "-o",
        &output_path,
        "--strict",
    ]);

    assert!(!output.status.success());
    assert!(!std::path::Path::new(&output_path).exists());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("ML_STYLE_SCALE"));
    assert!(stderr.contains("warning"));
}

#[test]
fn music_export_midi_produces_valid_file() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let output_path = format!("{workspace}/target/test-export.mid");
    let _ = fs::remove_file(&output_path);

    let output = run_music(&[
        "export",
        "examples/minimal.music",
        "--format",
        "midi",
        "-o",
        &output_path,
    ]);

    assert!(output.status.success());
    assert!(output.stdout.contains(&b'\n'));
    let bytes = fs::read(&output_path).unwrap();
    assert!(bytes.starts_with(b"MThd"));
}

#[test]
fn music_ir_advances_over_rest() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/rest.music");
    fs::write(
        &input_path,
        r#"
score demo {
  voice lead {
    note C4, 1/4
    rest 1/2
    note E4, 1/4
  }
}
"#,
    )
    .unwrap();

    let output = run_music(&["ir", &input_path]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("class: C"));
    assert!(stdout.contains("class: E"));
    assert!(stdout.contains("start_tick: 1440"));
}

#[test]
fn music_ir_expands_pedal_tone() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/pedal-tone.music");
    fs::write(
        &input_path,
        r#"
score demo {
  voice bass {
    pedal C3, 4, 1/4
  }
}
"#,
    )
    .unwrap();

    let output = run_music(&["ir", &input_path]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("class: C"));
    assert!(stdout.contains("octave: 3"));
    assert!(stdout.contains("start_tick: 1440"));
    assert!(stdout.contains("duration_ticks: 480"));
}

#[test]
fn music_ir_expands_scale_degree_notes() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/scale-degree.music");
    fs::write(
        &input_path,
        r#"
score demo {
  key C major
  voice lead {
    degree 1 4, 1/8
    degree b3 4, 1/8
    modulate G major
    degree 1 4, 1/8
  }
}
"#,
    )
    .unwrap();

    let output = run_music(&["ir", &input_path]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("class: C"));
    assert!(stdout.contains("class: Ds"));
    assert!(stdout.contains("class: G"));
    assert!(stdout.contains("duration_ticks: 240"));
}

#[test]
fn music_ir_expands_scale_run() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/scale-run.music");
    fs::write(
        &input_path,
        r#"
score demo {
  voice lead {
    scale C major 4, 1/8
  }
}
"#,
    )
    .unwrap();

    let output = run_music(&["ir", &input_path]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("class: C"));
    assert!(stdout.contains("class: D"));
    assert!(stdout.contains("class: E"));
    assert!(stdout.contains("start_tick: 1680"));
    assert!(stdout.contains("duration_ticks: 240"));
}

#[test]
fn music_ir_expands_ostinato_block() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/ostinato.music");
    fs::write(
        &input_path,
        r#"
score demo {
  voice bass {
    ostinato 3 {
      note C3, 1/8
      note G3, 1/8
    }
  }
}
"#,
    )
    .unwrap();

    let output = run_music(&["ir", &input_path]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("class: C"));
    assert!(stdout.contains("class: G"));
    assert!(stdout.contains("start_tick: 1200"));
    assert!(stdout.contains("duration_ticks: 240"));
}

#[test]
fn music_ir_expands_transpose_block() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/transpose.music");
    fs::write(
        &input_path,
        r#"
score demo {
  voice lead {
    transpose M2 {
      note C4, 1/8
      chord [E4, G4], 1/8
    }
  }
}
"#,
    )
    .unwrap();

    let output = run_music(&["ir", &input_path]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("class: D"));
    assert!(stdout.contains("class: Fs"));
    assert!(stdout.contains("class: A"));
    assert!(stdout.contains("duration_ticks: 240"));
}

#[test]
fn music_ir_expands_sequence_block() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/sequence.music");
    fs::write(
        &input_path,
        r#"
score demo {
  voice lead {
    sequence 3 by M2 {
      note C4, 1/8
    }
  }
}
"#,
    )
    .unwrap();

    let output = run_music(&["ir", &input_path]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("class: C"));
    assert!(stdout.contains("class: D"));
    assert!(stdout.contains("class: E"));
    assert!(stdout.contains("start_tick: 480"));
    assert!(stdout.contains("duration_ticks: 240"));
}

#[test]
fn music_ir_scales_tuplet_block() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/tuplet.music");
    fs::write(
        &input_path,
        r#"
score demo {
  voice lead {
    tuplet 3 in 1/4 {
      note C4, 1/8
      rest 1/8
      chord [E4, G4], 1/8
    }
    note C5, 1/4
  }
}
"#,
    )
    .unwrap();

    let output = run_music(&["ir", &input_path]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("duration_ticks: 160"));
    assert!(stdout.contains("start_tick: 320"));
    assert!(stdout.contains("start_tick: 480"));
    assert!(stdout.contains("duration_ticks: 480"));
}

#[test]
fn music_ir_expands_glissando_as_stepped_notes() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/glissando.music");
    fs::write(
        &input_path,
        r#"
score demo {
  voice lead {
    glissando C4 to G4 steps 5, 1/16
  }
}
"#,
    )
    .unwrap();

    let output = run_music(&["ir", &input_path]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("class: C"));
    assert!(stdout.contains("class: Cs"));
    assert!(stdout.contains("class: Ds"));
    assert!(stdout.contains("class: F"));
    assert!(stdout.contains("class: G"));
    assert!(stdout.contains("start_tick: 120"));
    assert!(stdout.contains("start_tick: 480"));
    assert!(stdout.contains("duration_ticks: 120"));
}

#[test]
fn music_ir_expands_tremolo_as_alternating_notes() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/tremolo.music");
    fs::write(
        &input_path,
        r#"
score demo {
  voice strings {
    tremolo C4 with G4 repeats 4, 1/32
  }
}
"#,
    )
    .unwrap();

    let output = run_music(&["ir", &input_path]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("class: C"));
    assert!(stdout.contains("class: G"));
    assert!(stdout.contains("start_tick: 60"));
    assert!(stdout.contains("start_tick: 180"));
    assert!(stdout.contains("duration_ticks: 60"));
}

#[test]
fn music_ir_expands_strum_as_staggered_notes() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/strum.music");
    fs::write(
        &input_path,
        r#"
score demo {
  voice guitar {
    strum [C4, E4, G4], 1/2 by 1/32
    strum C4 dominant7 inv 2, 1/2 by 1/64
  }
}
"#,
    )
    .unwrap();

    let output = run_music(&["ir", &input_path]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("class: C"));
    assert!(stdout.contains("class: E"));
    assert!(stdout.contains("class: G"));
    assert!(stdout.contains("class: As"));
    assert!(stdout.contains("start_tick: 60"));
    assert!(stdout.contains("start_tick: 120"));
    assert!(stdout.contains("start_tick: 1050"));
    assert!(stdout.contains("duration_ticks: 960"));
}

#[test]
fn music_ir_expands_arpeggio_as_sequential_notes() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/arpeggio.music");
    fs::write(
        &input_path,
        r#"
score demo {
  voice lead {
    arpeggio [C4, E4, G4], 1/8
    arpeggio D3 minor inv 1, 1/16
    transpose M2 {
      arpeggio [C4, E4, G4], 1/8
    }
  }
}
"#,
    )
    .unwrap();

    let output = run_music(&["ir", &input_path]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("class: C"));
    assert!(stdout.contains("class: E"));
    assert!(stdout.contains("class: G"));
    assert!(stdout.contains("class: D"));
    assert!(stdout.contains("class: F"));
    assert!(stdout.contains("class: A"));
    assert!(stdout.contains("class: Fs"));
    assert!(stdout.contains("start_tick: 0"));
    assert!(stdout.contains("start_tick: 240"));
    assert!(stdout.contains("start_tick: 480"));
    assert!(stdout.contains("duration_ticks: 120"));
}

#[test]
fn music_ir_expands_named_chord_quality() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/named-chord.music");
    fs::write(
        &input_path,
        r#"
score demo {
  voice lead {
    chord D3 minor7 inv 1, 1/2
  }
}
"#,
    )
    .unwrap();

    let output = run_music(&["ir", &input_path]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("class: F"));
    assert!(stdout.contains("class: A"));
    assert!(stdout.contains("class: C"));
    assert!(stdout.contains("class: D"));
    assert!(stdout.contains("octave: 4"));
    assert!(stdout.contains("duration_ticks: 960"));
}

#[test]
fn music_ir_expands_roman_numeral_chord() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/roman-chord.music");
    fs::write(
        &input_path,
        r#"
score demo {
  key C major
  voice lead {
    roman V65/V, 1/2
    roman bVII, 1/4
    roman viidim/V, 1/4
  }
}
"#,
    )
    .unwrap();

    let output = run_music(&["ir", &input_path]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("class: Fs"));
    assert!(stdout.contains("class: As"));
    assert!(stdout.contains("class: D"));
    assert!(stdout.contains("class: F"));
    assert!(stdout.contains("duration_ticks: 960"));
}

#[test]
fn music_ir_expands_harmonic_progression() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/harmonic-progression.music");
    fs::write(
        &input_path,
        r#"
score demo {
  key C major
  voice lead {
    progression I vi ii V7 I, 1/4
  }
}
"#,
    )
    .unwrap();

    let output = run_music(&["ir", &input_path]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("class: C"));
    assert!(stdout.contains("class: A"));
    assert!(stdout.contains("class: B"));
    assert!(stdout.contains("start_tick: 1920"));
    assert!(stdout.contains("duration_ticks: 480"));
}

#[test]
fn music_ir_expands_named_cadence() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/named-cadence.music");
    fs::write(
        &input_path,
        r#"
score demo {
  key C major
  voice lead {
    cadence authentic, 1/2
  }
}
"#,
    )
    .unwrap();

    let output = run_music(&["ir", &input_path]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("class: G"));
    assert!(stdout.contains("class: B"));
    assert!(stdout.contains("class: C"));
    assert!(stdout.contains("start_tick: 960"));
    assert!(stdout.contains("duration_ticks: 960"));
}

#[test]
fn music_ir_applies_modulation_to_roman_chords() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/modulation.music");
    fs::write(
        &input_path,
        r#"
score demo {
  key C major
  voice lead {
    roman I, 1/4
    modulate G major
    roman I, 1/4
  }
}
"#,
    )
    .unwrap();

    let output = run_music(&["ir", &input_path]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("class: G"));
    assert!(stdout.contains("class: B"));
    assert!(stdout.contains("class: D"));
    assert!(stdout.contains("octave: 5"));
    assert!(stdout.contains("start_tick: 480"));
}

#[test]
fn music_export_musicxml_produces_valid_file() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = write_analyze_metadata_fixture();
    let output_path = format!("{workspace}/target/test-export.musicxml");
    let _ = fs::remove_file(&output_path);

    let output = run_music(&[
        "export",
        &input_path,
        "--format",
        "musicxml",
        "-o",
        &output_path,
    ]);

    assert!(output.status.success());
    let xml = fs::read_to_string(&output_path).unwrap();
    assert!(xml.starts_with("<?xml"));
    assert!(xml.contains("<score-partwise"));
    assert!(xml.contains("<work-title>String Quartet</work-title>"));
    assert!(xml.contains("<creator type=\"composer\">Ada Lovelace</creator>"));
    assert!(xml.contains("<fifths>-1</fifths>"));
    assert!(xml.contains("<mode>minor</mode>"));
}

#[test]
fn music_export_wav_produces_valid_file() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let output_path = format!("{workspace}/target/test-export.wav");
    let _ = fs::remove_file(&output_path);

    let output = run_music(&[
        "export",
        "examples/minimal.music",
        "--format",
        "wav",
        "-o",
        &output_path,
    ]);

    assert!(output.status.success());
    let bytes = fs::read(&output_path).unwrap();
    assert!(bytes.starts_with(b"RIFF"));
    assert_eq!(&bytes[8..12], b"WAVE");
}

#[test]
fn music_ast_prints_parsed_program() {
    let output = run_music(&["ast", "examples/minimal.music"]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Program"));
    assert!(stdout.contains("ScoreDecl"));
}

#[test]
fn music_ir_prints_lowered_score() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/ir-track-metadata.music");
    fs::write(
        &input_path,
        r#"
score ir_track_metadata {
  tempo 144
  meter 6/8
  key G major
  voice lead {
    section Theme {
      tempo 144
      meter 6/8
      key G major
    }
    program 40
    volume 96
    pan 32
    note C4, 1/4
  }
}
"#,
    )
    .unwrap();
    let output = run_music(&["ir", &input_path]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ScoreIr"));
    assert!(stdout.contains("tempo_bpm"));
    assert!(stdout.contains("program: Some(\n                40,"));
    assert!(stdout.contains("volume: Some(\n                96,"));
    assert!(stdout.contains("pan: Some(\n                32,"));
    assert!(stdout.contains("tempo_changes"));
    assert!(stdout.contains("bpm: 144"));
    assert!(stdout.contains("meter_changes"));
    assert!(stdout.contains("numerator: 6"));
    assert!(stdout.contains("denominator: 8"));
    assert!(stdout.contains("key_changes"));
    assert!(stdout.contains("fifths: 1"));
    assert!(stdout.contains("markers"));
    assert!(stdout.contains("Theme"));
}

fn write_analyze_metadata_fixture() -> String {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/analyze-metadata.music");
    fs::write(
        &input_path,
        r#"
score demo {
  title "String Quartet"
  composer "Ada Lovelace"
  tempo 96
  meter 3/4
  key D minor
  voice lead {
    note F4, 1/4
    note G4, 1/4
  }
  voice alto {
    note A3, 1/4
  }
  voice bass {
    note D3, 1/2
  }
}
"#,
    )
    .unwrap();
    input_path
}

#[test]
fn music_analyze_summarizes_score() {
    let input_path = write_analyze_metadata_fixture();
    let output = run_music(&["analyze", &input_path]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("title: String Quartet"));
    assert!(stdout.contains("composer: Ada Lovelace"));
    assert!(stdout.contains("tempo: 96 bpm"));
    assert!(stdout.contains("meter: 3/4"));
    assert!(stdout.contains("key: D minor"));
    assert!(stdout.contains("tracks: 3"));
    assert!(stdout.contains("events: 4"));
    assert!(stdout.contains("duration_ticks: 960"));
    assert!(stdout.contains("bar_ticks: 1440"));
    assert!(stdout.contains("duration_bars: 1"));
    assert!(stdout.contains("density_per_bar: 4"));
    assert!(stdout.contains("repeated_bar_count: 0"));
    assert!(stdout.contains("repeated_bar_ratio_percent: 0"));
    assert!(stdout.contains("longest_repeated_bar_run: 1"));
    assert!(stdout.contains("max_simultaneous_events: 3"));
    assert!(stdout.contains("texture: dense_polyphonic"));
    assert!(stdout.contains("pitch_range: D3..G4"));
    assert!(stdout.contains("pitch_classes: A,D,F,G"));
    assert!(stdout.contains("roman_roots: biii,i,iv,v"));
    assert!(stdout.contains("sonority tick=0: pcs=D,F,A, root=D, quality=minor, roman=i"));
    assert!(stdout.contains("track lead: events=2, density_per_bar=2, range=F4..G4"));
    assert!(stdout.contains("track alto: events=1, density_per_bar=1, range=A3..A3"));
    assert!(stdout.contains("track bass: events=1, density_per_bar=1, range=D3..D3"));
}

#[test]
fn music_analyze_reports_repeated_bars() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/analyze-repetition.music");
    fs::write(
        &input_path,
        r#"
score repetitive {
  tempo 120
  meter 4/4
  voice lead {
    note C4, 1/4
    note E4, 1/4
    note G4, 1/4
    note E4, 1/4
    note C4, 1/4
    note E4, 1/4
    note G4, 1/4
    note E4, 1/4
    note C4, 1/4
    note E4, 1/4
    note G4, 1/4
    note E4, 1/4
    note C4, 1/4
    note E4, 1/4
    note G4, 1/4
    note E4, 1/4
  }
}
"#,
    )
    .unwrap();
    let output = run_music(&["analyze", &input_path]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("duration_bars: 4"));
    assert!(stdout.contains("repeated_bar_count: 3"));
    assert!(stdout.contains("repeated_bar_ratio_percent: 75"));
    assert!(stdout.contains("longest_repeated_bar_run: 4"));
}

#[test]
fn music_analyze_strict_rejects_repeated_arrangement() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/analyze-strict-repetition.music");
    fs::write(
        &input_path,
        r#"
score repetitive {
  tempo 120
  meter 4/4
  voice lead {
    note C4, 1/4
    note E4, 1/4
    note G4, 1/4
    note E4, 1/4
    note C4, 1/4
    note E4, 1/4
    note G4, 1/4
    note E4, 1/4
    note C4, 1/4
    note E4, 1/4
    note G4, 1/4
    note E4, 1/4
    note C4, 1/4
    note E4, 1/4
    note G4, 1/4
    note E4, 1/4
  }
}
"#,
    )
    .unwrap();
    let output = run_music(&["analyze", &input_path, "--strict"]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("analysis quality gate failed"));
    assert!(stderr.contains("repeated_bar_ratio_percent 75 exceeds 50"));
}

#[test]
fn music_analyze_strict_rejects_warning_only_diagnostics() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/analyze-strict-warning.music");
    fs::write(
        &input_path,
        r#"
style WarningScale {
  scale: C major
  severity_scale: warning
}

score warning_only style WarningScale {
  key C major
  voice lead {
    note C4, 1/4
    note F#4, 1/4
  }
}
"#,
    )
    .unwrap();
    let output = run_music(&["analyze", &input_path, "--strict"]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("analysis quality gate failed"));
    assert!(stderr.contains("diagnostics 1 exceeds 0"));
}

#[test]
fn music_analyze_strict_accepts_demo() {
    let output = run_music(&["analyze", "examples/demo_jazz_complete.music", "--strict"]);

    assert!(output.status.success());
}

#[test]
fn music_analyze_json_is_machine_readable() {
    let input_path = write_analyze_metadata_fixture();
    let output = run_music(&["analyze", &input_path, "--json"]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"title\":\"String Quartet\""));
    assert!(stdout.contains("\"composer\":\"Ada Lovelace\""));
    assert!(stdout.contains("\"tempo_bpm\":96"));
    assert!(stdout.contains("\"meter\":{\"numerator\":3,\"denominator\":4}"));
    assert!(stdout.contains("\"key\":{\"tonic\":\"D\",\"mode\":\"minor\",\"fifths\":-1}"));
    assert!(stdout.contains("\"track_count\":3"));
    assert!(stdout.contains("\"event_count\":4"));
    assert!(stdout.contains("\"duration_ticks\":960"));
    assert!(stdout.contains("\"bar_ticks\":1440"));
    assert!(stdout.contains("\"duration_bars\":1"));
    assert!(stdout.contains("\"density_per_bar\":4"));
    assert!(stdout.contains("\"repeated_bar_count\":0"));
    assert!(stdout.contains("\"repeated_bar_ratio_percent\":0"));
    assert!(stdout.contains("\"longest_repeated_bar_run\":1"));
    assert!(stdout.contains("\"max_simultaneous_events\":3"));
    assert!(stdout.contains("\"texture\":\"dense_polyphonic\""));
    assert!(stdout.contains("\"pitch_min\":\"D3\""));
    assert!(stdout.contains("\"pitch_max\":\"G4\""));
    assert!(stdout.contains("\"pitch_classes\":[\"A\",\"D\",\"F\",\"G\"]"));
    assert!(stdout.contains("\"roman_roots\":[\"biii\",\"i\",\"iv\",\"v\"]"));
    assert!(stdout.contains("\"sonorities\":[{\"tick\":0,\"pitch_classes\":[\"D\",\"F\",\"A\"],\"root\":\"D\",\"quality\":\"minor\",\"roman\":\"i\"}]"));
    assert!(stdout.contains(
        "\"tracks\":[{\"name\":\"lead\",\"event_count\":2,\"density_per_bar\":2,\"pitch_min\":\"F4\",\"pitch_max\":\"G4\"},{\"name\":\"alto\",\"event_count\":1,\"density_per_bar\":1,\"pitch_min\":\"A3\",\"pitch_max\":\"A3\"},{\"name\":\"bass\",\"event_count\":1,\"density_per_bar\":1,\"pitch_min\":\"D3\",\"pitch_max\":\"D3\"}]"
    ));
}

#[test]
fn music_export_rejects_unknown_format() {
    let output = run_music(&[
        "export",
        "examples/minimal.music",
        "--format",
        "tracker",
        "-o",
        "target/test-export.tracker",
    ]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unsupported export format `tracker`"));
}

#[test]
fn music_check_reports_error_on_violation() {
    let output = run_music(&["check", "examples/style_violation.music"]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("ML_STYLE_SCALE"));
}

#[test]
fn music_repl_loads_diagnoses_shows_ir_and_exports() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let output_path = format!("{workspace}/target/repl-export.mid");
    let _ = fs::remove_file(&output_path);
    let script = format!(
        ":load examples/minimal.music\n:diagnose\n:show ir\n:export {output_path}\n:quit\n"
    );

    let output = run_music_with_stdin(&["repl"], &script);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("loaded examples/minimal.music"));
    assert!(stdout.contains("ok"));
    assert!(stdout.contains("ScoreIr"));
    assert!(stdout.contains(&format!("wrote {output_path}")));
    assert!(fs::read(&output_path).unwrap().starts_with(b"MThd"));
}

#[test]
fn music_repl_reset_clears_source_buffer() {
    let script =
        "score demo {\n  voice lead {\n    note C4, 1/4\n  }\n}\n:reset\n:show source\n:quit\n";

    let output = run_music_with_stdin(&["repl"], script);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("reset"));
    let after_reset = stdout.split("reset").last().unwrap();
    assert!(!after_reset.contains("score demo"));
}

#[test]
fn music_diagnose_reports_ok_for_valid_override() {
    let output = run_music(&["diagnose", "examples/override.music"]);

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "ok");
    assert!(output.stderr.is_empty());
}

#[test]
fn music_check_reports_unknown_function_as_resolve_error() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/unknown-function.music");
    fs::write(
        &input_path,
        r#"
score demo {
  voice lead {
    call missing
  }
}
"#,
    )
    .unwrap();

    let output = run_music(&["check", &input_path]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("ML_RESOLVE_UNKNOWN_NAME"));
    assert!(stderr.contains("unknown function `missing`"));
}

#[test]
fn music_diagnose_json_reports_recursive_call() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/recursive-call.music");
    fs::write(
        &input_path,
        r#"
fn motif {
  call motif
}
score demo {
  voice lead {
    call motif
  }
}
"#,
    )
    .unwrap();

    let output = run_music(&["diagnose", &input_path, "--json"]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"code\":\"ML_RESOLVE_RECURSIVE_CALL\""));
    assert!(stdout.contains("recursive function call `motif`"));
}

#[test]
fn music_diagnose_json_reports_compiler_span() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/json-unknown-function.music");
    let source = r#"
score demo {
  voice lead {
    call missing
  }
}
"#;
    fs::write(&input_path, source).unwrap();

    let output = run_music(&["diagnose", &input_path, "--json"]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let expected_start = source.find("call missing").unwrap();
    let expected_end = expected_start + "call".len();
    assert!(stdout.contains("\"code\":\"ML_RESOLVE_UNKNOWN_NAME\""));
    assert!(stdout.contains(&format!("\"start\":{expected_start}")));
    assert!(stdout.contains(&format!("\"end\":{expected_end}")));
}
