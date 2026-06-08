use std::f32::consts::TAU;
use std::io;

use musiclang_core::{PitchClass, ScoreIr};

pub fn render_musicxml(score: &ScoreIr) -> String {
    let mut output = String::new();
    output.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    output.push_str("<!DOCTYPE score-partwise PUBLIC \"-//Recordare//DTD MusicXML 4.0 Partwise//EN\" \"http://www.musicxml.org/dtds/partwise.dtd\">\n");
    output.push_str("<score-partwise version=\"4.0\">\n");
    output.push_str(&format!(
        "  <work><work-title>{}</work-title></work>\n",
        escape_xml(&score.title)
    ));
    if let Some(composer) = &score.composer {
        output.push_str(&format!(
            "  <identification><creator type=\"composer\">{}</creator></identification>\n",
            escape_xml(composer)
        ));
    }
    output.push_str("  <part-list>\n");
    for (index, track) in score.tracks.iter().enumerate() {
        output.push_str(&format!(
            "    <score-part id=\"P{}\"><part-name>{}</part-name></score-part>\n",
            index + 1,
            escape_xml(&track.name)
        ));
    }
    output.push_str("  </part-list>\n");
    for (index, track) in score.tracks.iter().enumerate() {
        output.push_str(&format!("  <part id=\"P{}\">\n", index + 1));
        output.push_str("    <measure number=\"1\">\n");
        output.push_str("      <attributes>\n");
        output.push_str(&format!(
            "        <divisions>{}</divisions>\n",
            score.ticks_per_quarter
        ));
        if let Some(meter) = score.meter {
            output.push_str("        <time>\n");
            output.push_str(&format!("          <beats>{}</beats>\n", meter.numerator));
            output.push_str(&format!(
                "          <beat-type>{}</beat-type>\n",
                meter.denominator
            ));
            output.push_str("        </time>\n");
        }
        if let Some(key) = score.key {
            output.push_str("        <key>\n");
            output.push_str(&format!("          <fifths>{}</fifths>\n", key.fifths));
            output.push_str(&format!(
                "          <mode>{}</mode>\n",
                if key.is_minor { "minor" } else { "major" }
            ));
            output.push_str("        </key>\n");
        }
        output.push_str("      </attributes>\n");
        output.push_str(&format!(
            "      <direction><sound tempo=\"{}\"/></direction>\n",
            score.tempo_bpm
        ));
        for event in &track.events {
            output.push_str("      <note>\n");
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
            output.push_str(&format!(
                "        <duration>{}</duration>\n",
                event.duration_ticks
            ));
            if let Some(articulation) = event
                .articulation
                .as_deref()
                .and_then(musicxml_articulation)
            {
                output.push_str("        <notations><articulations>");
                output.push_str(&format!("<{articulation}/>"));
                output.push_str("</articulations></notations>\n");
            }
            output.push_str("      </note>\n");
        }
        output.push_str("    </measure>\n");
        output.push_str("  </part>\n");
    }
    output.push_str("</score-partwise>\n");
    output
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
    use musiclang_core::{
        KeySignature, NoteEventIr, Pitch, PitchClass, ScoreIr, TrackIr, DEFAULT_TICKS_PER_QUARTER,
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
            tracks: vec![TrackIr {
                name: "lead".to_string(),
                channel: 0,
                program: None,
                volume: None,
                pan: None,
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
            overrides: Vec::new(),
        }
    }

    #[test]
    fn renders_musicxml() {
        let xml = render_musicxml(&score());

        assert!(xml.contains("<score-partwise"));
        assert!(xml.contains("<part-name>lead</part-name>"));
        assert!(xml.contains("<creator type=\"composer\">Ada Lovelace</creator>"));
        assert!(xml.contains("<fifths>-1</fifths>"));
        assert!(xml.contains("<mode>major</mode>"));
        assert!(xml.contains("<staccato/>"));
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
