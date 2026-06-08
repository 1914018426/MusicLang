use std::f32::consts::TAU;
use std::io;

use musiclang_core::{Meter, NoteEventIr, PitchClass, ScoreIr};

pub fn render_musicxml(score: &ScoreIr) -> String {
    let mut output = String::new();
    output.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    output.push_str("<!DOCTYPE score-partwise PUBLIC \"-//Recordare//DTD MusicXML 4.0 Partwise//EN\" \"http://www.musicxml.org/dtds/partwise.dtd\">\n");
    output.push_str("<score-partwise version=\"4.0\">\n");
    output.push_str(&format!(
        "  <work><work-title>{}</work-title></work>\n",
        escape_xml(&score.title)
    ));
    if score.composer.is_some() || !score.metadata.is_empty() {
        render_musicxml_identification(&mut output, score);
    }
    output.push_str("  <part-list>\n");
    for (index, track) in score.tracks.iter().enumerate() {
        let part_id = format!("P{}", index + 1);
        let instrument_id = format!("{part_id}-I1");
        output.push_str(&format!("    <score-part id=\"{part_id}\">\n"));
        output.push_str(&format!(
            "      <part-name>{}</part-name>\n",
            escape_xml(&track.name)
        ));
        output.push_str(&format!(
            "      <score-instrument id=\"{instrument_id}\"><instrument-name>{}</instrument-name></score-instrument>\n",
            escape_xml(&track.name)
        ));
        output.push_str(&format!("      <midi-instrument id=\"{instrument_id}\">\n"));
        output.push_str(&format!(
            "        <midi-channel>{}</midi-channel>\n",
            track.channel.min(15) + 1
        ));
        if let Some(program) = track.program {
            output.push_str(&format!(
                "        <midi-program>{}</midi-program>\n",
                program.min(127) + 1
            ));
        }
        if let Some(volume) = track.volume {
            output.push_str(&format!("        <volume>{}</volume>\n", volume.min(100)));
        }
        if let Some(pan) = track.pan {
            output.push_str(&format!("        <pan>{}</pan>\n", musicxml_pan(pan)));
        }
        output.push_str("      </midi-instrument>\n");
        output.push_str("    </score-part>\n");
    }
    output.push_str("  </part-list>\n");
    let bar_ticks = musicxml_bar_ticks(score.ticks_per_quarter, score.meter);
    let measure_count = musicxml_measure_count(score, bar_ticks);
    for (index, track) in score.tracks.iter().enumerate() {
        output.push_str(&format!("  <part id=\"P{}\">\n", index + 1));
        render_musicxml_track(&mut output, score, track, bar_ticks, measure_count);
        output.push_str("  </part>\n");
    }
    output.push_str("</score-partwise>\n");
    output
}

fn render_musicxml_identification(output: &mut String, score: &ScoreIr) {
    output.push_str("  <identification>\n");
    if let Some(composer) = &score.composer {
        output.push_str(&format!(
            "    <creator type=\"composer\">{}</creator>\n",
            escape_xml(composer)
        ));
    }
    if !score.metadata.is_empty() {
        output.push_str("    <miscellaneous>\n");
        for (key, value) in &score.metadata {
            output.push_str(&format!(
                "      <miscellaneous-field name=\"{}\">{}</miscellaneous-field>\n",
                escape_xml(key),
                escape_xml(value)
            ));
        }
        output.push_str("    </miscellaneous>\n");
    }
    output.push_str("  </identification>\n");
}

fn render_musicxml_track(
    output: &mut String,
    score: &ScoreIr,
    track: &musiclang_core::TrackIr,
    bar_ticks: u32,
    measure_count: u32,
) {
    let mut events: Vec<&NoteEventIr> = track.events.iter().collect();
    events.sort_by_key(|event| {
        (
            event.start_tick,
            event.pitch.octave(),
            event.pitch.class().semitone(),
        )
    });
    for measure_index in 0..measure_count {
        let measure_start = measure_index * bar_ticks;
        let measure_end = measure_start + bar_ticks;
        output.push_str(&format!("    <measure number=\"{}\">\n", measure_index + 1));
        if measure_index == 0 {
            render_musicxml_initial_attributes(output, score);
        }
        render_musicxml_timeline_changes(output, score, measure_start, measure_end);
        let mut cursor_tick = measure_start;
        let mut previous_start = None;
        for event in events.iter().copied().filter(|event| {
            event.start_tick < measure_end
                && event.start_tick + event.duration_ticks > measure_start
        }) {
            let event_start = event.start_tick.max(measure_start);
            let event_end = (event.start_tick + event.duration_ticks).min(measure_end);
            let original_end = event.start_tick + event.duration_ticks;
            let tie_stop = event_start > event.start_tick;
            let tie_start = event_end < original_end;
            if Some(event.start_tick) != previous_start {
                if event_start > cursor_tick {
                    render_musicxml_rest(output, event_start - cursor_tick);
                }
                cursor_tick = cursor_tick.max(event_end);
                previous_start = Some(event.start_tick);
                render_musicxml_note_with_duration(
                    output,
                    event,
                    event_end - event_start,
                    false,
                    tie_start,
                    tie_stop,
                );
            } else {
                cursor_tick = cursor_tick.max(event_end);
                render_musicxml_note_with_duration(
                    output,
                    event,
                    event_end - event_start,
                    true,
                    tie_start,
                    tie_stop,
                );
            }
        }
        if cursor_tick < measure_end {
            render_musicxml_rest(output, measure_end - cursor_tick);
        }
        output.push_str("    </measure>\n");
    }
}

fn render_musicxml_initial_attributes(output: &mut String, score: &ScoreIr) {
    render_musicxml_attributes(
        output,
        Some(score.ticks_per_quarter),
        score.meter,
        score.key,
    );
    render_musicxml_tempo(output, score.tempo_bpm);
}

fn render_musicxml_timeline_changes(
    output: &mut String,
    score: &ScoreIr,
    measure_start: u32,
    measure_end: u32,
) {
    for change in &score.meter_changes {
        if change.tick >= measure_start && change.tick < measure_end {
            render_musicxml_attributes(output, None, Some(change.meter), None);
        }
    }
    for change in &score.key_changes {
        if change.tick >= measure_start && change.tick < measure_end {
            render_musicxml_attributes(output, None, None, Some(change.key));
        }
    }
    for change in &score.tempo_changes {
        if change.tick >= measure_start && change.tick < measure_end {
            render_musicxml_tempo(output, change.bpm);
        }
    }
    for marker in &score.markers {
        if marker.tick >= measure_start && marker.tick < measure_end {
            render_musicxml_marker(output, &marker.label);
        }
    }
    render_musicxml_semantic_events(output, score, measure_start, measure_end);
}

fn render_musicxml_attributes(
    output: &mut String,
    divisions: Option<u32>,
    meter: Option<Meter>,
    key: Option<musiclang_core::KeySignature>,
) {
    output.push_str("      <attributes>\n");
    if let Some(divisions) = divisions {
        output.push_str(&format!("        <divisions>{divisions}</divisions>\n"));
    }
    if let Some(meter) = meter {
        output.push_str("        <time>\n");
        output.push_str(&format!("          <beats>{}</beats>\n", meter.numerator));
        output.push_str(&format!(
            "          <beat-type>{}</beat-type>\n",
            meter.denominator
        ));
        output.push_str("        </time>\n");
    }
    if let Some(key) = key {
        output.push_str("        <key>\n");
        output.push_str(&format!("          <fifths>{}</fifths>\n", key.fifths));
        output.push_str(&format!(
            "          <mode>{}</mode>\n",
            if key.is_minor { "minor" } else { "major" }
        ));
        output.push_str("        </key>\n");
    }
    output.push_str("      </attributes>\n");
}

fn render_musicxml_tempo(output: &mut String, tempo_bpm: u16) {
    output.push_str(&format!(
        "      <direction><sound tempo=\"{}\"/></direction>\n",
        tempo_bpm
    ));
}

fn render_musicxml_marker(output: &mut String, label: &str) {
    output.push_str(&format!(
        "      <direction><direction-type><words>{}</words></direction-type></direction>\n",
        escape_xml(label)
    ));
}

fn render_musicxml_semantic_events(
    output: &mut String,
    score: &ScoreIr,
    measure_start: u32,
    measure_end: u32,
) {
    for event in &score.form_events {
        if event.start_tick >= measure_start && event.start_tick < measure_end {
            render_musicxml_marker(output, &format!("form {} {}", event.kind, event.label));
        }
    }
    for event in &score.phrase_events {
        if event.start_tick >= measure_start && event.start_tick < measure_end {
            let label = event.label.as_deref().unwrap_or("unlabeled");
            render_musicxml_marker(output, &format!("phrase {} {label}", event.kind));
        }
    }
    for event in &score.motif_events {
        if event.start_tick >= measure_start && event.start_tick < measure_end {
            let transform = event.transform.as_deref().unwrap_or("literal");
            render_musicxml_marker(output, &format!("motif {} {transform}", event.name));
        }
    }
    for event in &score.harmonic_events {
        if event.start_tick >= measure_start && event.start_tick < measure_end {
            render_musicxml_marker(output, &format!("harmony {}", event.normalized_symbol));
        }
    }
    for event in &score.melodic_events {
        if event.start_tick >= measure_start && event.start_tick < measure_end {
            let degree = event
                .degree
                .map(|degree| degree.to_string())
                .unwrap_or_else(|| "unknown".to_string());
            render_musicxml_marker(output, &format!("melody {} degree {degree}", event.kind));
        }
    }
}

fn render_musicxml_rest(output: &mut String, duration_ticks: u32) {
    output.push_str("      <note>\n");
    output.push_str("        <rest/>\n");
    output.push_str(&format!("        <duration>{duration_ticks}</duration>\n"));
    output.push_str("      </note>\n");
}

fn musicxml_bar_ticks(ticks_per_quarter: u32, meter: Option<Meter>) -> u32 {
    let meter = meter.unwrap_or_default();
    ticks_per_quarter * u32::from(meter.numerator) * 4 / u32::from(meter.denominator)
}

fn musicxml_measure_count(score: &ScoreIr, bar_ticks: u32) -> u32 {
    let event_ticks = score
        .tracks
        .iter()
        .flat_map(|track| track.events.iter())
        .map(|event| event.start_tick + event.duration_ticks)
        .max()
        .unwrap_or(0);
    let timeline_ticks = score
        .tempo_changes
        .iter()
        .map(|change| change.tick)
        .chain(score.meter_changes.iter().map(|change| change.tick))
        .chain(score.key_changes.iter().map(|change| change.tick))
        .chain(score.markers.iter().map(|marker| marker.tick))
        .chain(score.form_events.iter().map(|event| event.start_tick))
        .chain(score.phrase_events.iter().map(|event| event.start_tick))
        .chain(score.motif_events.iter().map(|event| event.start_tick))
        .chain(score.harmonic_events.iter().map(|event| event.start_tick))
        .chain(score.melodic_events.iter().map(|event| event.start_tick))
        .max()
        .unwrap_or(0);
    let total_ticks = event_ticks.max(timeline_ticks.saturating_add(1));
    (total_ticks.max(1).saturating_sub(1) / bar_ticks) + 1
}

fn musicxml_pan(pan: u8) -> i16 {
    ((i16::from(pan.min(127)) * 180) / 127) - 90
}

fn render_musicxml_note_with_duration(
    output: &mut String,
    event: &NoteEventIr,
    duration_ticks: u32,
    chord: bool,
    tie_start: bool,
    tie_stop: bool,
) {
    output.push_str("      <note>\n");
    if chord {
        output.push_str("        <chord/>\n");
    }
    output.push_str("        <pitch>\n");
    output.push_str(&format!(
        "          <step>{}</step>\n",
        pitch_step(event.pitch.class())
    ));
    if let Some(alter) = pitch_alter(event.pitch.class()) {
        output.push_str(&format!("          <alter>{alter}</alter>\n"));
    }
    output.push_str(&format!(
        "          <octave>{}</octave>\n",
        event.pitch.octave()
    ));
    output.push_str("        </pitch>\n");
    output.push_str(&format!("        <duration>{duration_ticks}</duration>\n"));
    if tie_stop {
        output.push_str("        <tie type=\"stop\"/>\n");
    }
    if tie_start {
        output.push_str("        <tie type=\"start\"/>\n");
    }
    let articulation = event
        .articulation
        .as_deref()
        .and_then(musicxml_articulation);
    if tie_start || tie_stop || articulation.is_some() {
        output.push_str("        <notations>");
        if tie_stop {
            output.push_str("<tied type=\"stop\"/>");
        }
        if tie_start {
            output.push_str("<tied type=\"start\"/>");
        }
        if let Some(articulation) = articulation {
            output.push_str("<articulations>");
            output.push_str(&format!("<{articulation}/>"));
            output.push_str("</articulations>");
        }
        output.push_str("</notations>\n");
    }
    output.push_str("      </note>\n");
}

pub fn render_wav(score: &ScoreIr) -> io::Result<Vec<u8>> {
    let sample_rate = 44_100u32;
    let total_ticks = score
        .tracks
        .iter()
        .flat_map(|track| track.events.iter())
        .map(|event| event.start_tick + event.duration_ticks)
        .max()
        .unwrap_or(0);
    let seconds_per_tick =
        60.0 / f32::from(score.tempo_bpm.max(1)) / score.ticks_per_quarter as f32;
    let sample_count =
        ((total_ticks as f32 * seconds_per_tick + 0.25) * sample_rate as f32) as usize;
    let mut samples = vec![(0.0f32, 0.0f32); sample_count.max(1)];

    for track in &score.tracks {
        let volume = f32::from(track.volume.unwrap_or(100).min(127)) / 127.0;
        let pan = f32::from(track.pan.unwrap_or(64).min(127)) / 127.0;
        let left_gain = (1.0 - pan).sqrt() * volume;
        let right_gain = pan.sqrt() * volume;
        for event in &track.events {
            let midi = event.pitch.midi_number().map_err(io::Error::other)?;
            let start = (event.start_tick as f32 * seconds_per_tick * sample_rate as f32) as usize;
            let len =
                (event.duration_ticks as f32 * seconds_per_tick * sample_rate as f32) as usize;
            for i in 0..len.min(samples.len().saturating_sub(start)) {
                let sample = if track.channel == 9 {
                    drum_sample(midi, i, len.max(1), sample_rate)
                } else {
                    pitched_sample(midi, track.program, i, len.max(1), sample_rate)
                } * f32::from(event.velocity.min(127))
                    / 127.0;
                samples[start + i].0 += sample * left_gain;
                samples[start + i].1 += sample * right_gain;
            }
        }
    }

    let mut bytes = Vec::new();
    let data_len = samples.len() as u32 * 4;
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(&(sample_rate * 4).to_le_bytes());
    bytes.extend_from_slice(&4u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    for (left, right) in samples {
        let left = (left.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16;
        let right = (right.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16;
        bytes.extend_from_slice(&left.to_le_bytes());
        bytes.extend_from_slice(&right.to_le_bytes());
    }
    Ok(bytes)
}

fn pitched_sample(
    midi: u8,
    program: Option<u8>,
    index: usize,
    len: usize,
    sample_rate: u32,
) -> f32 {
    let frequency = 440.0 * 2f32.powf((f32::from(midi) - 69.0) / 12.0);
    let t = index as f32 / sample_rate as f32;
    let phase = TAU * frequency * t;
    let envelope = melodic_envelope(index, len);
    let sample = match program.unwrap_or(0) {
        0..=7 => phase.sin() * 0.16 + (phase * 2.0).sin() * 0.04,
        24..=31 => guitar_wave(phase) * 0.14,
        32..=39 => phase.sin() * 0.18 + (phase * 0.5).sin() * 0.08,
        40..=55 => phase.sin() * 0.12 + (phase * 1.01).sin() * 0.08,
        56..=71 => brass_wave(phase) * 0.16,
        72..=79 => phase.sin() * 0.12 + (phase * 3.0).sin() * 0.02,
        88..=95 => phase.sin() * 0.10 + (phase * 0.501).sin() * 0.08,
        _ => phase.sin() * 0.16,
    };
    sample * envelope
}

fn drum_sample(midi: u8, index: usize, len: usize, sample_rate: u32) -> f32 {
    let t = index as f32 / sample_rate as f32;
    let envelope = (1.0 - index as f32 / len as f32).max(0.0).powf(4.0);
    let noise = deterministic_noise(index as u32 ^ (u32::from(midi) * 8191));
    match midi {
        35 | 36 => (TAU * (80.0 - 45.0 * t.min(1.0)) * t).sin() * envelope * 0.55,
        37..=39 => (noise * 0.42 + (TAU * 190.0 * t).sin() * 0.16) * envelope,
        42 | 44 | 46 => noise * envelope.powf(0.55) * 0.26,
        49 | 51 => noise * envelope.powf(0.35) * 0.34,
        45 | 47 | 50 => (TAU * 150.0 * t).sin() * envelope * 0.35,
        _ => noise * envelope * 0.22,
    }
}

fn melodic_envelope(index: usize, len: usize) -> f32 {
    let attack = (index as f32 / (len as f32 * 0.05).max(1.0)).min(1.0);
    let release = (1.0 - index as f32 / len as f32).max(0.0);
    attack * release.powf(0.8)
}

fn guitar_wave(phase: f32) -> f32 {
    (phase.sin() + (phase * 2.0).sin() * 0.35 + (phase * 3.0).sin() * 0.16) * 0.7
}

fn brass_wave(phase: f32) -> f32 {
    (phase.sin() + (phase * 2.0).sin() * 0.28 + (phase * 3.0).sin() * 0.12).tanh()
}

fn deterministic_noise(seed: u32) -> f32 {
    let value = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    ((value >> 16) as f32 / 32_768.0) * 2.0 - 1.0
}

fn pitch_step(class: PitchClass) -> &'static str {
    match class {
        PitchClass::C | PitchClass::Cs => "C",
        PitchClass::D | PitchClass::Ds => "D",
        PitchClass::E => "E",
        PitchClass::F | PitchClass::Fs => "F",
        PitchClass::G | PitchClass::Gs => "G",
        PitchClass::A | PitchClass::As => "A",
        PitchClass::B => "B",
    }
}

fn pitch_alter(class: PitchClass) -> Option<i8> {
    match class {
        PitchClass::Cs | PitchClass::Ds | PitchClass::Fs | PitchClass::Gs | PitchClass::As => {
            Some(1)
        }
        _ => None,
    }
}

fn musicxml_articulation(articulation: &str) -> Option<&'static str> {
    match articulation {
        "staccato" => Some("staccato"),
        "tenuto" => Some("tenuto"),
        "accent" => Some("accent"),
        "legato" => Some("legato"),
        _ => None,
    }
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use musiclang_core::{
        FormEventIr, HarmonicEventIr, KeyChangeIr, KeySignature, MarkerIr, MelodicEventIr,
        MeterChangeIr, MotifEventIr, NoteEventIr, PhraseEventIr, Pitch, PitchClass, ScoreIr,
        TempoChangeIr, TrackIr, DEFAULT_TICKS_PER_QUARTER,
    };

    use super::*;

    fn score() -> ScoreIr {
        ScoreIr {
            title: "demo".to_string(),
            composer: Some("Ada Lovelace".to_string()),
            ticks_per_quarter: DEFAULT_TICKS_PER_QUARTER,
            tempo_bpm: 120,
            meter: None,
            key: Some(KeySignature {
                fifths: -1,
                is_minor: false,
            }),
            metadata: BTreeMap::new(),
            tracks: vec![TrackIr {
                name: "lead".to_string(),
                channel: 1,
                program: Some(40),
                volume: Some(96),
                pan: Some(64),
                events: vec![NoteEventIr {
                    pitch: Pitch::new(PitchClass::C, 4).unwrap(),
                    start_tick: 0,
                    duration_ticks: DEFAULT_TICKS_PER_QUARTER,
                    velocity: 80,
                    articulation: Some("staccato".to_string()),
                    source_span: None,
                }],
            }],
            markers: Vec::new(),
            tempo_changes: Vec::new(),
            meter_changes: Vec::new(),
            key_changes: Vec::new(),
            harmonic_events: Vec::new(),
            melodic_events: Vec::new(),
            form_events: Vec::new(),
            motif_events: Vec::new(),
            phrase_events: Vec::new(),
            overrides: Vec::new(),
        }
    }

    #[test]
    fn renders_musicxml() {
        let xml = render_musicxml(&score());

        assert!(xml.contains("<score-partwise"));
        assert!(xml.contains("<part-name>lead</part-name>"));
        assert!(xml.contains("<score-instrument id=\"P1-I1\"><instrument-name>lead</instrument-name></score-instrument>"));
        assert!(xml.contains("<midi-channel>2</midi-channel>"));
        assert!(xml.contains("<midi-program>41</midi-program>"));
        assert!(xml.contains("<volume>96</volume>"));
        assert!(xml.contains("<pan>0</pan>"));
        assert!(xml.contains("<creator type=\"composer\">Ada Lovelace</creator>"));
        assert!(xml.contains("<fifths>-1</fifths>"));
        assert!(xml.contains("<mode>major</mode>"));
        assert!(xml.contains("<staccato/>"));
    }

    #[test]
    fn musicxml_escapes_metadata_and_part_names() {
        let mut score = score();
        score.title = "A&B <Suite> \"I\"".to_string();
        score.composer = Some("Ada & Bob's <Duo>".to_string());
        score
            .metadata
            .insert("session & take".to_string(), "A <B> \"C\"".to_string());
        score.tracks[0].name = "lead & \"alto\" <top>".to_string();

        let xml = render_musicxml(&score);

        assert!(xml.contains("<work-title>A&amp;B &lt;Suite&gt; &quot;I&quot;</work-title>"));
        assert!(
            xml.contains("<creator type=\"composer\">Ada &amp; Bob&apos;s &lt;Duo&gt;</creator>")
        );
        assert!(xml.contains(
            "<miscellaneous-field name=\"session &amp; take\">A &lt;B&gt; &quot;C&quot;</miscellaneous-field>"
        ));
        assert!(xml.contains("<part-name>lead &amp; &quot;alto&quot; &lt;top&gt;</part-name>"));
        assert!(!xml.contains("A&B <Suite>"));
        assert!(!xml.contains("lead & \"alto\" <top>"));
    }

    #[test]
    fn musicxml_renders_semantic_event_directions() {
        let mut score = score();
        score.form_events = vec![FormEventIr {
            label: "A&B".to_string(),
            kind: "section".to_string(),
            start_tick: 0,
            duration_ticks: DEFAULT_TICKS_PER_QUARTER,
            source_span: None,
        }];
        score.phrase_events = vec![PhraseEventIr {
            label: Some("theme".to_string()),
            kind: "motif_call".to_string(),
            start_tick: 0,
            duration_ticks: DEFAULT_TICKS_PER_QUARTER,
            source_span: None,
        }];
        score.motif_events = vec![MotifEventIr {
            name: "motif".to_string(),
            transform: Some("transposition".to_string()),
            start_tick: 0,
            duration_ticks: DEFAULT_TICKS_PER_QUARTER,
            source_span: None,
        }];
        score.harmonic_events = vec![HarmonicEventIr {
            symbol: "V7".to_string(),
            normalized_symbol: "V7".to_string(),
            degree: Some(5),
            applied_to: None,
            function: Some("dominant".to_string()),
            cadence_role: None,
            start_tick: 0,
            duration_ticks: DEFAULT_TICKS_PER_QUARTER,
            source_span: None,
        }];
        score.melodic_events = vec![MelodicEventIr {
            kind: "scale_degree".to_string(),
            degree: Some(3),
            accidental: -1,
            pitch: Pitch::new(PitchClass::E, 4).unwrap(),
            start_tick: 0,
            duration_ticks: DEFAULT_TICKS_PER_QUARTER,
            source_span: None,
        }];

        let xml = render_musicxml(&score);

        assert!(xml.contains("form section A&amp;B"));
        assert!(xml.contains("phrase motif_call theme"));
        assert!(xml.contains("motif motif transposition"));
        assert!(xml.contains("harmony V7"));
        assert!(xml.contains("melody scale_degree degree 3"));
    }

    #[test]
    fn musicxml_renders_rests_for_gaps_and_chord_notes() {
        let mut score = score();
        score.tracks[0].events = vec![
            NoteEventIr {
                pitch: Pitch::new(PitchClass::C, 4).unwrap(),
                start_tick: DEFAULT_TICKS_PER_QUARTER,
                duration_ticks: DEFAULT_TICKS_PER_QUARTER,
                velocity: 80,
                articulation: None,
                source_span: None,
            },
            NoteEventIr {
                pitch: Pitch::new(PitchClass::E, 4).unwrap(),
                start_tick: DEFAULT_TICKS_PER_QUARTER,
                duration_ticks: DEFAULT_TICKS_PER_QUARTER,
                velocity: 80,
                articulation: None,
                source_span: None,
            },
        ];

        let xml = render_musicxml(&score);

        assert!(xml.contains("<rest/>"));
        assert!(xml.contains(&format!(
            "<duration>{}</duration>",
            DEFAULT_TICKS_PER_QUARTER
        )));
        assert!(xml.contains("<chord/>"));
    }

    #[test]
    fn musicxml_renders_timeline_changes() {
        let mut score = score();
        score.meter = Some(musiclang_core::Meter {
            numerator: 4,
            denominator: 4,
        });
        score.tempo_changes = vec![TempoChangeIr {
            bpm: 144,
            tick: DEFAULT_TICKS_PER_QUARTER * 4,
        }];
        score.meter_changes = vec![MeterChangeIr {
            meter: musiclang_core::Meter {
                numerator: 3,
                denominator: 4,
            },
            tick: DEFAULT_TICKS_PER_QUARTER * 4,
        }];
        score.key_changes = vec![KeyChangeIr {
            key: KeySignature {
                fifths: 2,
                is_minor: true,
            },
            tick: DEFAULT_TICKS_PER_QUARTER * 4,
        }];
        score.tracks[0].events = vec![NoteEventIr {
            pitch: Pitch::new(PitchClass::C, 4).unwrap(),
            start_tick: 0,
            duration_ticks: DEFAULT_TICKS_PER_QUARTER,
            velocity: 80,
            articulation: None,
            source_span: None,
        }];

        let xml = render_musicxml(&score);

        assert_eq!(xml.matches("<measure number=").count(), 2);
        assert!(xml.contains("<beats>4</beats>"));
        assert!(xml.contains("<beats>3</beats>"));
        assert!(xml.contains("<fifths>-1</fifths>"));
        assert!(xml.contains("<fifths>2</fifths>"));
        assert!(xml.contains("<mode>minor</mode>"));
        assert!(xml.contains("<sound tempo=\"120\"/>"));
        assert!(xml.contains("<sound tempo=\"144\"/>"));
    }

    #[test]
    fn musicxml_renders_markers_and_marker_only_measures() {
        let mut score = score();
        score.meter = Some(musiclang_core::Meter {
            numerator: 4,
            denominator: 4,
        });
        score.markers = vec![MarkerIr {
            label: "Bridge & Release".to_string(),
            tick: DEFAULT_TICKS_PER_QUARTER * 4,
        }];
        score.tracks[0].events = vec![NoteEventIr {
            pitch: Pitch::new(PitchClass::C, 4).unwrap(),
            start_tick: 0,
            duration_ticks: DEFAULT_TICKS_PER_QUARTER,
            velocity: 80,
            articulation: None,
            source_span: None,
        }];

        let xml = render_musicxml(&score);

        assert_eq!(xml.matches("<measure number=").count(), 2);
        assert!(xml.contains("<words>Bridge &amp; Release</words>"));
    }

    #[test]
    fn musicxml_splits_parts_into_measures() {
        let mut score = score();
        score.meter = Some(musiclang_core::Meter {
            numerator: 3,
            denominator: 4,
        });
        score.tracks[0].events = vec![
            NoteEventIr {
                pitch: Pitch::new(PitchClass::C, 4).unwrap(),
                start_tick: 0,
                duration_ticks: DEFAULT_TICKS_PER_QUARTER,
                velocity: 80,
                articulation: None,
                source_span: None,
            },
            NoteEventIr {
                pitch: Pitch::new(PitchClass::D, 4).unwrap(),
                start_tick: DEFAULT_TICKS_PER_QUARTER * 3,
                duration_ticks: DEFAULT_TICKS_PER_QUARTER,
                velocity: 80,
                articulation: None,
                source_span: None,
            },
        ];

        let xml = render_musicxml(&score);

        assert!(xml.contains("<measure number=\"1\">"));
        assert!(xml.contains("<measure number=\"2\">"));
        assert_eq!(xml.matches("<measure number=").count(), 2);
        assert_eq!(xml.matches("<attributes>").count(), 1);
    }

    #[test]
    fn musicxml_aligns_part_measures_and_fills_trailing_rests() {
        let mut score = score();
        score.meter = Some(musiclang_core::Meter {
            numerator: 3,
            denominator: 4,
        });
        score.tracks[0].events = vec![NoteEventIr {
            pitch: Pitch::new(PitchClass::C, 4).unwrap(),
            start_tick: 0,
            duration_ticks: DEFAULT_TICKS_PER_QUARTER,
            velocity: 80,
            articulation: None,
            source_span: None,
        }];
        score.tracks.push(TrackIr {
            name: "bass".to_string(),
            channel: 1,
            program: None,
            volume: None,
            pan: None,
            events: vec![NoteEventIr {
                pitch: Pitch::new(PitchClass::G, 3).unwrap(),
                start_tick: DEFAULT_TICKS_PER_QUARTER * 3,
                duration_ticks: DEFAULT_TICKS_PER_QUARTER,
                velocity: 80,
                articulation: None,
                source_span: None,
            }],
        });

        let xml = render_musicxml(&score);

        assert_eq!(xml.matches("<measure number=\"1\">").count(), 2);
        assert_eq!(xml.matches("<measure number=\"2\">").count(), 2);
        assert_eq!(xml.matches("<attributes>").count(), 2);
        assert!(xml.matches("<rest/>").count() >= 3);
    }

    #[test]
    fn musicxml_splits_notes_at_measure_boundaries() {
        let mut score = score();
        score.meter = Some(musiclang_core::Meter {
            numerator: 3,
            denominator: 4,
        });
        score.tracks[0].events = vec![NoteEventIr {
            pitch: Pitch::new(PitchClass::C, 4).unwrap(),
            start_tick: DEFAULT_TICKS_PER_QUARTER * 2,
            duration_ticks: DEFAULT_TICKS_PER_QUARTER * 2,
            velocity: 80,
            articulation: None,
            source_span: None,
        }];

        let xml = render_musicxml(&score);

        assert_eq!(xml.matches("<measure number=").count(), 2);
        assert_eq!(xml.matches("<duration>480</duration>").count(), 2);
        assert_eq!(xml.matches("<tie type=\"start\"/>").count(), 1);
        assert_eq!(xml.matches("<tie type=\"stop\"/>").count(), 1);
        assert_eq!(xml.matches("<tied type=\"start\"/>").count(), 1);
        assert_eq!(xml.matches("<tied type=\"stop\"/>").count(), 1);
    }

    #[test]
    fn renders_wav() {
        let wav = render_wav(&score()).unwrap();

        assert!(wav.starts_with(b"RIFF"));
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(u16::from_le_bytes([wav[22], wav[23]]), 2);
        assert_eq!(u16::from_le_bytes([wav[32], wav[33]]), 4);
    }

    #[test]
    fn wav_rendering_applies_pan() {
        let mut left_score = score();
        left_score.tracks[0].pan = Some(0);
        let left = render_wav(&left_score).unwrap();
        let mut right_score = score();
        right_score.tracks[0].pan = Some(127);
        let right = render_wav(&right_score).unwrap();

        let left_energy = channel_energy(&left, 0);
        let right_energy = channel_energy(&right, 1);

        assert!(left_energy > channel_energy(&left, 1) * 4.0);
        assert!(right_energy > channel_energy(&right, 0) * 4.0);
    }

    fn channel_energy(wav: &[u8], channel: usize) -> f64 {
        wav[44..]
            .chunks_exact(4)
            .map(|frame| {
                let offset = channel * 2;
                let sample = i16::from_le_bytes([frame[offset], frame[offset + 1]]) as f64;
                sample.abs()
            })
            .sum()
    }
}
