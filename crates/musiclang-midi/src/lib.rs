use std::io::{self, Write};

use midly::{
    num::{u15, u24, u28, u4, u7},
    Format, Header, MetaMessage, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind,
};
use musiclang_core::ScoreIr;

pub fn write_midi<W: Write>(score: &ScoreIr, writer: &mut W) -> io::Result<()> {
    let mut tracks = Vec::new();
    let microseconds_per_quarter = 60_000_000 / u32::from(score.tempo_bpm.max(1));
    let mut tempo_track = vec![TrackEvent {
        delta: u28::new(0),
        kind: TrackEventKind::Meta(MetaMessage::TrackName(score.title.as_bytes())),
    }];
    if let Some(composer) = &score.composer {
        tempo_track.push(TrackEvent {
            delta: u28::new(0),
            kind: TrackEventKind::Meta(MetaMessage::Text(composer.as_bytes())),
        });
    }
    tempo_track.push(TrackEvent {
        delta: u28::new(0),
        kind: TrackEventKind::Meta(MetaMessage::Tempo(u24::new(microseconds_per_quarter))),
    });
    if let Some(meter) = score.meter {
        tempo_track.push(TrackEvent {
            delta: u28::new(0),
            kind: TrackEventKind::Meta(MetaMessage::TimeSignature(
                meter.numerator,
                meter.denominator.trailing_zeros() as u8,
                24,
                8,
            )),
        });
    }
    if let Some(key) = score.key {
        tempo_track.push(TrackEvent {
            delta: u28::new(0),
            kind: TrackEventKind::Meta(MetaMessage::KeySignature(key.fifths, key.is_minor)),
        });
    }
    tempo_track.push(TrackEvent {
        delta: u28::new(0),
        kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
    });
    tracks.push(tempo_track);

    for track in &score.tracks {
        let channel = u4::new(track.channel.min(15));
        let mut absolute_events = Vec::new();
        if let Some(program) = track.program {
            absolute_events.push((
                0,
                TrackEventKind::Midi {
                    channel,
                    message: MidiMessage::ProgramChange {
                        program: u7::new(program.min(127)),
                    },
                },
            ));
        }
        for event in &track.events {
            let key = u7::new(event.pitch.midi_number().map_err(io::Error::other)?);
            let velocity = articulated_velocity(event.velocity, event.articulation.as_deref());
            let duration_ticks =
                articulated_duration(event.duration_ticks, event.articulation.as_deref());
            absolute_events.push((
                event.start_tick,
                TrackEventKind::Midi {
                    channel,
                    message: MidiMessage::NoteOn {
                        key,
                        vel: u7::new(velocity),
                    },
                },
            ));
            absolute_events.push((
                event.start_tick + duration_ticks,
                TrackEventKind::Midi {
                    channel,
                    message: MidiMessage::NoteOff {
                        key,
                        vel: u7::new(0),
                    },
                },
            ));
        }
        absolute_events.sort_by_key(|(tick, kind)| (*tick, event_order(kind)));

        let mut midi_track = vec![TrackEvent {
            delta: u28::new(0),
            kind: TrackEventKind::Meta(MetaMessage::TrackName(track.name.as_bytes())),
        }];
        let mut cursor = 0;
        for (tick, kind) in absolute_events {
            midi_track.push(TrackEvent {
                delta: u28::new(tick - cursor),
                kind,
            });
            cursor = tick;
        }
        midi_track.push(TrackEvent {
            delta: u28::new(0),
            kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
        });
        tracks.push(midi_track);
    }

    let smf = Smf {
        header: Header {
            format: Format::Parallel,
            timing: Timing::Metrical(u15::new(score.ticks_per_quarter as u16)),
        },
        tracks,
    };
    smf.write_std(writer)
}

pub fn render_midi(score: &ScoreIr) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    write_midi(score, &mut bytes)?;
    Ok(bytes)
}

fn articulated_velocity(velocity: u8, articulation: Option<&str>) -> u8 {
    match articulation {
        Some("accent") => velocity.saturating_add(16).min(127),
        _ => velocity.min(127),
    }
}

fn articulated_duration(duration_ticks: u32, articulation: Option<&str>) -> u32 {
    match articulation {
        Some("staccato") => (duration_ticks / 2).max(1),
        _ => duration_ticks,
    }
}

fn event_order(kind: &TrackEventKind<'_>) -> u8 {
    match kind {
        TrackEventKind::Midi {
            message: MidiMessage::NoteOff { .. },
            ..
        } => 0,
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use midly::TrackEventKind;
    use musiclang_core::{
        KeySignature, Meter, NoteEventIr, Pitch, PitchClass, ScoreIr, TrackIr,
        DEFAULT_TICKS_PER_QUARTER,
    };

    #[test]
    fn renders_midi_bytes() {
        let score = ScoreIr {
            title: "demo".to_string(),
            composer: Some("Ada Lovelace".to_string()),
            ticks_per_quarter: DEFAULT_TICKS_PER_QUARTER,
            tempo_bpm: 90,
            meter: Some(Meter {
                numerator: 3,
                denominator: 4,
            }),
            key: Some(KeySignature {
                fifths: -1,
                is_minor: false,
            }),
            tracks: vec![TrackIr {
                name: "lead".to_string(),
                channel: 2,
                program: Some(40),
                events: vec![NoteEventIr {
                    pitch: Pitch::new(PitchClass::C, 4).unwrap(),
                    start_tick: 0,
                    duration_ticks: DEFAULT_TICKS_PER_QUARTER,
                    velocity: 80,
                    articulation: Some("accent".to_string()),
                    source_span: None,
                }],
            }],
            markers: Vec::new(),
            overrides: Vec::new(),
        };
        let bytes = render_midi(&score).unwrap();
        let smf = Smf::parse(&bytes).unwrap();

        assert!(bytes.starts_with(b"MThd"));
        assert!(matches!(
            smf.header.timing,
            Timing::Metrical(ticks) if ticks.as_int() == DEFAULT_TICKS_PER_QUARTER as u16
        ));
        assert!(smf.tracks[0].iter().any(|event| matches!(
            event.kind,
            TrackEventKind::Meta(MetaMessage::TrackName(b"demo"))
        )));
        assert!(smf.tracks[0].iter().any(|event| matches!(
            event.kind,
            TrackEventKind::Meta(MetaMessage::Text(b"Ada Lovelace"))
        )));
        assert!(smf.tracks[0].iter().any(|event| matches!(
            event.kind,
            TrackEventKind::Meta(MetaMessage::Tempo(value)) if value.as_int() == 666_666
        )));
        assert!(smf.tracks[0].iter().any(|event| matches!(
            event.kind,
            TrackEventKind::Meta(MetaMessage::TimeSignature(3, 2, 24, 8))
        )));
        assert!(smf.tracks[0].iter().any(|event| matches!(
            event.kind,
            TrackEventKind::Meta(MetaMessage::KeySignature(-1, false))
        )));
        assert!(matches!(
            smf.tracks[1][0].kind,
            TrackEventKind::Meta(MetaMessage::TrackName(b"lead"))
        ));
        assert!(smf.tracks[1].iter().any(|event| matches!(
            event.kind,
            TrackEventKind::Midi {
                channel,
                message: MidiMessage::ProgramChange { program },
            } if channel.as_int() == 2 && program.as_int() == 40
        )));
        assert!(smf.tracks[1].iter().any(|event| matches!(
            event.kind,
            TrackEventKind::Midi {
                channel,
                message: MidiMessage::NoteOn { vel, .. },
            } if channel.as_int() == 2 && vel.as_int() == 96
        )));
    }
}
