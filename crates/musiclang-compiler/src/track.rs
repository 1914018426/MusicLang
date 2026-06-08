use std::collections::{HashMap, HashSet};

use musiclang_core::{
    Chord, Duration, Note, NoteEventIr, Pitch, Span, TrackIr, DEFAULT_TICKS_PER_QUARTER,
};

pub(super) struct TrackBuilder {
    name: String,
    program: Option<u8>,
    channel: Option<u8>,
    volume: Option<u8>,
    pan: Option<u8>,
    cursor_tick: u32,
    velocity: u8,
    articulation: Option<String>,
    events: Vec<NoteEventIr>,
    overridden_event_rules: HashMap<String, HashSet<u32>>,
    time_scales: Vec<(u32, u32)>,
}

impl TrackBuilder {
    pub(super) fn new(
        name: &str,
        program: Option<u8>,
        channel: Option<u8>,
        volume: Option<u8>,
        pan: Option<u8>,
    ) -> Self {
        Self {
            name: name.to_string(),
            program,
            channel,
            volume,
            pan,
            cursor_tick: 0,
            velocity: 80,
            articulation: None,
            events: Vec::new(),
            overridden_event_rules: HashMap::new(),
            time_scales: Vec::new(),
        }
    }

    pub(super) fn scaled_ticks(&self, duration: Duration) -> u32 {
        let mut ticks = u64::from(duration.ticks(DEFAULT_TICKS_PER_QUARTER));
        for (count, space_ticks) in &self.time_scales {
            ticks = ticks * u64::from(*space_ticks) / (u64::from(*count) * 240);
        }
        ticks.max(1).min(u64::from(u32::MAX)) as u32
    }

    pub(super) fn push_time_scale(&mut self, count: u32, space_ticks: u32) {
        self.time_scales.push((count, space_ticks));
    }

    pub(super) fn pop_time_scale(&mut self) {
        self.time_scales.pop();
    }

    pub(super) fn advance(&mut self, duration: Duration) {
        self.cursor_tick += self.scaled_ticks(duration);
    }

    pub(super) fn push_note(&mut self, note: Note, source_span: Option<Span>) {
        let duration_ticks = self.scaled_ticks(note.duration());
        self.events.push(NoteEventIr {
            pitch: note.pitch(),
            start_tick: self.cursor_tick,
            duration_ticks,
            velocity: self.velocity,
            articulation: self.articulation.clone(),
            source_span,
        });
        self.cursor_tick += duration_ticks;
    }

    pub(super) fn push_midi_note(
        &mut self,
        midi: u8,
        duration: Duration,
        source_span: Option<Span>,
    ) {
        let duration_ticks = self.scaled_ticks(duration);
        self.events.push(NoteEventIr {
            pitch: Pitch::from_midi_number(i16::from(midi)).expect("valid GM drum note"),
            start_tick: self.cursor_tick,
            duration_ticks,
            velocity: self.velocity,
            articulation: self.articulation.clone(),
            source_span,
        });
        self.cursor_tick += duration_ticks;
    }

    pub(super) fn set_velocity(&mut self, velocity: u8) {
        self.velocity = velocity.min(127);
    }

    pub(super) fn set_articulation(&mut self, articulation: &str) {
        self.articulation = Some(articulation.to_string());
    }

    pub(super) fn program(&self) -> Option<u8> {
        self.program
    }

    pub(super) fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub(super) fn event_count(&self) -> usize {
        self.events.len()
    }

    pub(super) fn events(&self) -> &[NoteEventIr] {
        &self.events
    }

    pub(super) fn cursor_tick(&self) -> u32 {
        self.cursor_tick
    }

    pub(super) fn is_event_overridden(&self, rule: &str, start_tick: u32) -> bool {
        self.overridden_event_rules
            .get(rule)
            .is_some_and(|ticks| ticks.contains(&start_tick))
    }

    pub(super) fn mark_rule_override(&mut self, start_event: usize, rule: &str) {
        let ticks = self
            .overridden_event_rules
            .entry(rule.to_string())
            .or_default();
        for event in &self.events[start_event..] {
            ticks.insert(event.start_tick);
        }
    }

    pub(super) fn push_chord(&mut self, chord: Chord, source_span: Option<Span>) {
        let duration_ticks = self.scaled_ticks(chord.duration());
        for pitch in chord.pitches() {
            self.events.push(NoteEventIr {
                pitch: *pitch,
                start_tick: self.cursor_tick,
                duration_ticks,
                velocity: self.velocity,
                articulation: self.articulation.clone(),
                source_span,
            });
        }
        self.cursor_tick += duration_ticks;
    }

    pub(super) fn push_strum(
        &mut self,
        pitches: &[Pitch],
        duration: Duration,
        offset: Duration,
        source_span: Option<Span>,
    ) {
        let duration_ticks = self.scaled_ticks(duration);
        let offset_ticks = self.scaled_ticks(offset);
        for (index, pitch) in pitches.iter().enumerate() {
            self.events.push(NoteEventIr {
                pitch: *pitch,
                start_tick: self.cursor_tick + offset_ticks * index as u32,
                duration_ticks,
                velocity: self.velocity,
                articulation: self.articulation.clone(),
                source_span,
            });
        }
        self.cursor_tick += duration_ticks;
    }

    pub(super) fn finish(self) -> TrackIr {
        TrackIr {
            name: self.name,
            channel: self.channel.unwrap_or(0),
            program: self.program,
            volume: self.volume,
            pan: self.pan,
            events: self.events,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tuplet_time_scale_affects_note_duration_and_cursor() {
        let mut track = TrackBuilder::new("lead", None, None, None, None);
        let duration = Duration::new(1, 4).unwrap();
        let pitch = Pitch::from_midi_number(60).unwrap();
        let note = Note::new(pitch, duration);

        track.push_time_scale(3, 960);
        track.push_note(note, None);
        track.pop_time_scale();

        assert_eq!(track.events()[0].duration_ticks, 640);
        assert_eq!(track.cursor_tick(), 640);
    }

    #[test]
    fn mark_rule_override_applies_to_new_events_only() {
        let mut track = TrackBuilder::new("lead", None, None, None, None);
        let duration = Duration::new(1, 4).unwrap();
        let note = Note::new(Pitch::from_midi_number(60).unwrap(), duration);
        track.push_note(note.clone(), None);
        let start_event = track.event_count();
        track.push_note(note, None);
        let start_tick = track.events()[1].start_tick;

        track.mark_rule_override(start_event, "scale");

        assert!(!track.is_event_overridden("scale", 0));
        assert!(track.is_event_overridden("scale", start_tick));
    }
}
