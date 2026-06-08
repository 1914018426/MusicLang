use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::ops::{Add, Sub};
use std::str::FromStr;

use thiserror::Error;

pub const DEFAULT_TICKS_PER_QUARTER: u32 = 480;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TheoryCatalog {
    pub intervals: Vec<TheoryEntry>,
    pub scales: Vec<TheoryEntry>,
    pub modes: Vec<TheoryEntry>,
    pub chord_qualities: Vec<TheoryEntry>,
    pub cadences: Vec<TheoryEntry>,
    pub meters: Vec<TheoryEntry>,
    pub rhythms: Vec<TheoryEntry>,
    pub dynamics: Vec<TheoryEntry>,
    pub forms: Vec<TheoryEntry>,
    pub textures: Vec<TheoryEntry>,
    pub ornaments: Vec<TheoryEntry>,
    pub contrapuntal_motions: Vec<TheoryEntry>,
    pub non_chord_tones: Vec<TheoryEntry>,
    pub harmonic_functions: Vec<TheoryEntry>,
    pub set_classes: Vec<TheoryEntry>,
    pub tuning_systems: Vec<TheoryEntry>,
    pub world_traditions: Vec<TheoryEntry>,
    pub style_eras: Vec<TheoryEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TheoryEntry {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub pattern: &'static [&'static str],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TheoryDomain {
    Intervals,
    Scales,
    Modes,
    ChordQualities,
    Cadences,
    Meters,
    Rhythms,
    Dynamics,
    Forms,
    Textures,
    Ornaments,
    ContrapuntalMotions,
    NonChordTones,
    HarmonicFunctions,
    SetClasses,
    TuningSystems,
    WorldTraditions,
    StyleEras,
}

impl TheoryCatalog {
    pub const fn domains() -> &'static [TheoryDomain] {
        &[
            TheoryDomain::Intervals,
            TheoryDomain::Scales,
            TheoryDomain::Modes,
            TheoryDomain::ChordQualities,
            TheoryDomain::Cadences,
            TheoryDomain::Meters,
            TheoryDomain::Rhythms,
            TheoryDomain::Dynamics,
            TheoryDomain::Forms,
            TheoryDomain::Textures,
            TheoryDomain::Ornaments,
            TheoryDomain::ContrapuntalMotions,
            TheoryDomain::NonChordTones,
            TheoryDomain::HarmonicFunctions,
            TheoryDomain::SetClasses,
            TheoryDomain::TuningSystems,
            TheoryDomain::WorldTraditions,
            TheoryDomain::StyleEras,
        ]
    }

    pub fn entries(&self, domain: TheoryDomain) -> &[TheoryEntry] {
        match domain {
            TheoryDomain::Intervals => &self.intervals,
            TheoryDomain::Scales => &self.scales,
            TheoryDomain::Modes => &self.modes,
            TheoryDomain::ChordQualities => &self.chord_qualities,
            TheoryDomain::Cadences => &self.cadences,
            TheoryDomain::Meters => &self.meters,
            TheoryDomain::Rhythms => &self.rhythms,
            TheoryDomain::Dynamics => &self.dynamics,
            TheoryDomain::Forms => &self.forms,
            TheoryDomain::Textures => &self.textures,
            TheoryDomain::Ornaments => &self.ornaments,
            TheoryDomain::ContrapuntalMotions => &self.contrapuntal_motions,
            TheoryDomain::NonChordTones => &self.non_chord_tones,
            TheoryDomain::HarmonicFunctions => &self.harmonic_functions,
            TheoryDomain::SetClasses => &self.set_classes,
            TheoryDomain::TuningSystems => &self.tuning_systems,
            TheoryDomain::WorldTraditions => &self.world_traditions,
            TheoryDomain::StyleEras => &self.style_eras,
        }
    }

    pub fn find(&self, id: &str) -> Option<(TheoryDomain, &TheoryEntry)> {
        Self::domains().iter().find_map(|domain| {
            self.entries(*domain)
                .iter()
                .find(|entry| entry.id == id)
                .map(|entry| (*domain, entry))
        })
    }
}

impl fmt::Display for TheoryDomain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Intervals => "intervals",
            Self::Scales => "scales",
            Self::Modes => "modes",
            Self::ChordQualities => "chord_qualities",
            Self::Cadences => "cadences",
            Self::Meters => "meters",
            Self::Rhythms => "rhythms",
            Self::Dynamics => "dynamics",
            Self::Forms => "forms",
            Self::Textures => "textures",
            Self::Ornaments => "ornaments",
            Self::ContrapuntalMotions => "contrapuntal_motions",
            Self::NonChordTones => "non_chord_tones",
            Self::HarmonicFunctions => "harmonic_functions",
            Self::SetClasses => "set_classes",
            Self::TuningSystems => "tuning_systems",
            Self::WorldTraditions => "world_traditions",
            Self::StyleEras => "style_eras",
        };
        f.write_str(value)
    }
}

impl FromStr for TheoryDomain {
    type Err = CoreError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "intervals" => Ok(Self::Intervals),
            "scales" => Ok(Self::Scales),
            "modes" => Ok(Self::Modes),
            "chord_qualities" => Ok(Self::ChordQualities),
            "cadences" => Ok(Self::Cadences),
            "meters" => Ok(Self::Meters),
            "rhythms" => Ok(Self::Rhythms),
            "dynamics" => Ok(Self::Dynamics),
            "forms" => Ok(Self::Forms),
            "textures" => Ok(Self::Textures),
            "ornaments" => Ok(Self::Ornaments),
            "contrapuntal_motions" => Ok(Self::ContrapuntalMotions),
            "non_chord_tones" => Ok(Self::NonChordTones),
            "harmonic_functions" => Ok(Self::HarmonicFunctions),
            "set_classes" => Ok(Self::SetClasses),
            "tuning_systems" => Ok(Self::TuningSystems),
            "world_traditions" => Ok(Self::WorldTraditions),
            "style_eras" => Ok(Self::StyleEras),
            _ => Err(CoreError::InvalidTheoryDomain(value.to_string())),
        }
    }
}

pub fn theory_catalog() -> TheoryCatalog {
    TheoryCatalog {
        intervals: vec![
            TheoryEntry {
                id: "P1",
                name: "perfect unison",
                description: "zero-semitone interval",
                pattern: &["0"],
            },
            TheoryEntry {
                id: "m2",
                name: "minor second",
                description: "one-semitone interval",
                pattern: &["1"],
            },
            TheoryEntry {
                id: "M2",
                name: "major second",
                description: "two-semitone interval",
                pattern: &["2"],
            },
            TheoryEntry {
                id: "m3",
                name: "minor third",
                description: "three-semitone interval",
                pattern: &["3"],
            },
            TheoryEntry {
                id: "M3",
                name: "major third",
                description: "four-semitone interval",
                pattern: &["4"],
            },
            TheoryEntry {
                id: "P4",
                name: "perfect fourth",
                description: "five-semitone interval",
                pattern: &["5"],
            },
            TheoryEntry {
                id: "TT",
                name: "tritone",
                description: "six-semitone interval",
                pattern: &["6"],
            },
            TheoryEntry {
                id: "P5",
                name: "perfect fifth",
                description: "seven-semitone interval",
                pattern: &["7"],
            },
            TheoryEntry {
                id: "m6",
                name: "minor sixth",
                description: "eight-semitone interval",
                pattern: &["8"],
            },
            TheoryEntry {
                id: "M6",
                name: "major sixth",
                description: "nine-semitone interval",
                pattern: &["9"],
            },
            TheoryEntry {
                id: "m7",
                name: "minor seventh",
                description: "ten-semitone interval",
                pattern: &["10"],
            },
            TheoryEntry {
                id: "M7",
                name: "major seventh",
                description: "eleven-semitone interval",
                pattern: &["11"],
            },
            TheoryEntry {
                id: "P8",
                name: "octave",
                description: "twelve-semitone interval",
                pattern: &["12"],
            },
        ],
        scales: vec![
            TheoryEntry {
                id: "major",
                name: "major scale",
                description: "diatonic major scale",
                pattern: &["2", "2", "1", "2", "2", "2", "1"],
            },
            TheoryEntry {
                id: "natural_minor",
                name: "natural minor scale",
                description: "diatonic minor scale",
                pattern: &["2", "1", "2", "2", "1", "2", "2"],
            },
            TheoryEntry {
                id: "harmonic_minor",
                name: "harmonic minor scale",
                description: "minor scale with raised seventh degree",
                pattern: &["2", "1", "2", "2", "1", "3", "1"],
            },
            TheoryEntry {
                id: "melodic_minor",
                name: "melodic minor scale",
                description: "ascending jazz melodic minor collection",
                pattern: &["2", "1", "2", "2", "2", "2", "1"],
            },
            TheoryEntry {
                id: "major_pentatonic",
                name: "major pentatonic scale",
                description: "five-note major pentatonic collection",
                pattern: &["2", "2", "3", "2", "3"],
            },
            TheoryEntry {
                id: "minor_pentatonic",
                name: "minor pentatonic scale",
                description: "five-note minor pentatonic collection",
                pattern: &["3", "2", "2", "3", "2"],
            },
            TheoryEntry {
                id: "blues",
                name: "blues scale",
                description: "minor pentatonic with chromatic blue note",
                pattern: &["3", "2", "1", "1", "3", "2"],
            },
            TheoryEntry {
                id: "whole_tone",
                name: "whole-tone scale",
                description: "symmetric six-note whole-step collection",
                pattern: &["2", "2", "2", "2", "2", "2"],
            },
            TheoryEntry {
                id: "octatonic_wh",
                name: "octatonic whole-half scale",
                description: "symmetric alternating whole-half diminished collection",
                pattern: &["2", "1", "2", "1", "2", "1", "2", "1"],
            },
            TheoryEntry {
                id: "octatonic_hw",
                name: "octatonic half-whole scale",
                description: "symmetric alternating half-whole diminished collection",
                pattern: &["1", "2", "1", "2", "1", "2", "1", "2"],
            },
            TheoryEntry {
                id: "chromatic",
                name: "chromatic scale",
                description: "twelve-tone equal-tempered aggregate",
                pattern: &["1", "1", "1", "1", "1", "1", "1", "1", "1", "1", "1", "1"],
            },
            TheoryEntry {
                id: "hirajoshi",
                name: "Hirajoshi scale",
                description: "Japanese pentatonic tuning collection",
                pattern: &["2", "1", "4", "1", "4"],
            },
            TheoryEntry {
                id: "in_sen",
                name: "In-sen scale",
                description: "Japanese pentatonic scale with semitone inflection",
                pattern: &["1", "4", "2", "3", "2"],
            },
            TheoryEntry {
                id: "pelog",
                name: "Pelog subset",
                description: "representative gamelan pelog-derived scalar subset",
                pattern: &["1", "2", "4", "1", "4"],
            },
            TheoryEntry {
                id: "slendro",
                name: "Slendro approximation",
                description: "five-note near-equidistant gamelan slendro approximation",
                pattern: &["2", "2", "3", "2", "3"],
            },
        ],
        modes: vec![
            TheoryEntry {
                id: "ionian",
                name: "Ionian",
                description: "major mode",
                pattern: &["W", "W", "H", "W", "W", "W", "H"],
            },
            TheoryEntry {
                id: "dorian",
                name: "Dorian",
                description: "minor mode with raised sixth",
                pattern: &["W", "H", "W", "W", "W", "H", "W"],
            },
            TheoryEntry {
                id: "phrygian",
                name: "Phrygian",
                description: "minor mode with lowered second",
                pattern: &["H", "W", "W", "W", "H", "W", "W"],
            },
            TheoryEntry {
                id: "lydian",
                name: "Lydian",
                description: "major mode with raised fourth",
                pattern: &["W", "W", "W", "H", "W", "W", "H"],
            },
            TheoryEntry {
                id: "mixolydian",
                name: "Mixolydian",
                description: "major mode with lowered seventh",
                pattern: &["W", "W", "H", "W", "W", "H", "W"],
            },
            TheoryEntry {
                id: "aeolian",
                name: "Aeolian",
                description: "natural minor mode",
                pattern: &["W", "H", "W", "W", "H", "W", "W"],
            },
            TheoryEntry {
                id: "locrian",
                name: "Locrian",
                description: "diminished mode",
                pattern: &["H", "W", "W", "H", "W", "W", "W"],
            },
        ],
        chord_qualities: vec![
            TheoryEntry {
                id: "major",
                name: "major triad",
                description: "root, major third, perfect fifth",
                pattern: &["0", "4", "7"],
            },
            TheoryEntry {
                id: "minor",
                name: "minor triad",
                description: "root, minor third, perfect fifth",
                pattern: &["0", "3", "7"],
            },
            TheoryEntry {
                id: "diminished",
                name: "diminished triad",
                description: "root, minor third, diminished fifth",
                pattern: &["0", "3", "6"],
            },
            TheoryEntry {
                id: "augmented",
                name: "augmented triad",
                description: "root, major third, augmented fifth",
                pattern: &["0", "4", "8"],
            },
            TheoryEntry {
                id: "dominant7",
                name: "dominant seventh",
                description: "major triad with minor seventh",
                pattern: &["0", "4", "7", "10"],
            },
            TheoryEntry {
                id: "major7",
                name: "major seventh",
                description: "major triad with major seventh",
                pattern: &["0", "4", "7", "11"],
            },
            TheoryEntry {
                id: "minor7",
                name: "minor seventh",
                description: "minor triad with minor seventh",
                pattern: &["0", "3", "7", "10"],
            },
        ],
        cadences: vec![
            TheoryEntry {
                id: "authentic",
                name: "authentic cadence",
                description: "dominant to tonic",
                pattern: &["V", "I"],
            },
            TheoryEntry {
                id: "plagal",
                name: "plagal cadence",
                description: "subdominant to tonic",
                pattern: &["IV", "I"],
            },
            TheoryEntry {
                id: "half",
                name: "half cadence",
                description: "phrase ending on dominant",
                pattern: &["*", "V"],
            },
            TheoryEntry {
                id: "deceptive",
                name: "deceptive cadence",
                description: "dominant to submediant",
                pattern: &["V", "vi"],
            },
        ],
        meters: vec![
            TheoryEntry {
                id: "2/4",
                name: "simple duple",
                description: "two quarter-note beats",
                pattern: &["strong", "weak"],
            },
            TheoryEntry {
                id: "3/4",
                name: "simple triple",
                description: "three quarter-note beats",
                pattern: &["strong", "weak", "weak"],
            },
            TheoryEntry {
                id: "4/4",
                name: "simple quadruple",
                description: "four quarter-note beats",
                pattern: &["strong", "weak", "medium", "weak"],
            },
            TheoryEntry {
                id: "6/8",
                name: "compound duple",
                description: "two dotted-quarter beats",
                pattern: &["strong", "weak", "weak", "medium", "weak", "weak"],
            },
        ],
        rhythms: vec![
            TheoryEntry {
                id: "syncopation",
                name: "syncopation",
                description: "accenting weak beats or offbeats",
                pattern: &["weak_accent", "offbeat"],
            },
            TheoryEntry {
                id: "hemiola",
                name: "hemiola",
                description: "three-against-two metric reinterpretation",
                pattern: &["3:2"],
            },
            TheoryEntry {
                id: "swing",
                name: "swing eighths",
                description: "uneven subdivision with long-short feel",
                pattern: &["2:1", "triplet_subdivision"],
            },
            TheoryEntry {
                id: "ostinato",
                name: "ostinato",
                description: "persistently repeated rhythmic or melodic cell",
                pattern: &["repeat", "cell"],
            },
        ],
        dynamics: vec![
            TheoryEntry {
                id: "ppp",
                name: "pianississimo",
                description: "extremely soft dynamic",
                pattern: &["16"],
            },
            TheoryEntry {
                id: "pp",
                name: "pianissimo",
                description: "very soft dynamic",
                pattern: &["32"],
            },
            TheoryEntry {
                id: "p",
                name: "piano",
                description: "soft dynamic",
                pattern: &["48"],
            },
            TheoryEntry {
                id: "mp",
                name: "mezzo piano",
                description: "moderately soft dynamic",
                pattern: &["64"],
            },
            TheoryEntry {
                id: "mf",
                name: "mezzo forte",
                description: "moderately loud dynamic",
                pattern: &["80"],
            },
            TheoryEntry {
                id: "f",
                name: "forte",
                description: "loud dynamic",
                pattern: &["96"],
            },
            TheoryEntry {
                id: "ff",
                name: "fortissimo",
                description: "very loud dynamic",
                pattern: &["112"],
            },
            TheoryEntry {
                id: "fff",
                name: "fortississimo",
                description: "extremely loud dynamic",
                pattern: &["127"],
            },
            TheoryEntry {
                id: "sfz",
                name: "sforzando",
                description: "sudden forceful accent",
                pattern: &["118"],
            },
        ],
        forms: vec![
            TheoryEntry {
                id: "binary",
                name: "binary form",
                description: "two-part form",
                pattern: &["A", "B"],
            },
            TheoryEntry {
                id: "ternary",
                name: "ternary form",
                description: "return form with contrasting middle",
                pattern: &["A", "B", "A"],
            },
            TheoryEntry {
                id: "sonata",
                name: "sonata form",
                description: "exposition, development, and recapitulation",
                pattern: &["exposition", "development", "recapitulation"],
            },
            TheoryEntry {
                id: "rondo",
                name: "rondo form",
                description: "recurring refrain alternating with episodes",
                pattern: &["A", "B", "A", "C", "A"],
            },
            TheoryEntry {
                id: "twelve_bar_blues",
                name: "twelve-bar blues",
                description: "twelve-measure tonic/subdominant/dominant cycle",
                pattern: &[
                    "I", "I", "I", "I", "IV", "IV", "I", "I", "V", "IV", "I", "V",
                ],
            },
        ],
        textures: vec![
            TheoryEntry {
                id: "monophony",
                name: "monophony",
                description: "single melodic line",
                pattern: &["one_voice"],
            },
            TheoryEntry {
                id: "homophony",
                name: "homophony",
                description: "melody with chordal support",
                pattern: &["melody", "accompaniment"],
            },
            TheoryEntry {
                id: "polyphony",
                name: "polyphony",
                description: "multiple independent voices",
                pattern: &["independent_voices"],
            },
            TheoryEntry {
                id: "heterophony",
                name: "heterophony",
                description: "simultaneous variants of one melody",
                pattern: &["shared_melody", "variation"],
            },
        ],
        ornaments: vec![
            TheoryEntry {
                id: "trill",
                name: "trill",
                description: "rapid alternation with neighboring pitch",
                pattern: &["upper_neighbor", "main_note"],
            },
            TheoryEntry {
                id: "mordent",
                name: "mordent",
                description: "single rapid neighbor-note turn",
                pattern: &["main", "neighbor", "main"],
            },
            TheoryEntry {
                id: "turn",
                name: "turn",
                description: "upper and lower neighbor ornament",
                pattern: &["upper", "main", "lower", "main"],
            },
            TheoryEntry {
                id: "appoggiatura",
                name: "appoggiatura",
                description: "accented dissonance resolving by step",
                pattern: &["accented_dissonance", "step_resolution"],
            },
            TheoryEntry {
                id: "staccato",
                name: "staccato",
                description: "short detached articulation",
                pattern: &["duration:50"],
            },
            TheoryEntry {
                id: "tenuto",
                name: "tenuto",
                description: "sustained articulation held near full value",
                pattern: &["duration:100"],
            },
            TheoryEntry {
                id: "accent",
                name: "accent",
                description: "emphasized attack articulation",
                pattern: &["velocity:+16"],
            },
            TheoryEntry {
                id: "legato",
                name: "legato",
                description: "smooth connected articulation",
                pattern: &["duration:100"],
            },
        ],
        contrapuntal_motions: vec![
            TheoryEntry {
                id: "parallel",
                name: "parallel motion",
                description: "voices move in the same direction by the same interval",
                pattern: &["same_direction", "same_interval"],
            },
            TheoryEntry {
                id: "similar",
                name: "similar motion",
                description: "voices move in the same direction by different intervals",
                pattern: &["same_direction", "different_interval"],
            },
            TheoryEntry {
                id: "contrary",
                name: "contrary motion",
                description: "voices move in opposite directions",
                pattern: &["opposite_direction"],
            },
            TheoryEntry {
                id: "oblique",
                name: "oblique motion",
                description: "one voice holds while another moves",
                pattern: &["held_voice", "moving_voice"],
            },
        ],
        non_chord_tones: vec![
            TheoryEntry {
                id: "passing_tone",
                name: "passing tone",
                description: "stepwise non-chord tone between chord tones",
                pattern: &["chord", "step", "non_chord", "step", "chord"],
            },
            TheoryEntry {
                id: "neighbor_tone",
                name: "neighbor tone",
                description: "step away from and back to the same chord tone",
                pattern: &["chord", "step", "non_chord", "step_back", "chord"],
            },
            TheoryEntry {
                id: "suspension",
                name: "suspension",
                description: "prepared dissonance resolving downward by step",
                pattern: &["preparation", "suspension", "resolution"],
            },
            TheoryEntry {
                id: "anticipation",
                name: "anticipation",
                description: "early arrival of a pitch from the following harmony",
                pattern: &["early_next_chord_tone"],
            },
        ],
        harmonic_functions: vec![
            TheoryEntry {
                id: "tonic",
                name: "tonic function",
                description: "stability and resolution center",
                pattern: &["I", "vi"],
            },
            TheoryEntry {
                id: "predominant",
                name: "predominant function",
                description: "prepares dominant harmony",
                pattern: &["ii", "IV"],
            },
            TheoryEntry {
                id: "dominant",
                name: "dominant function",
                description: "creates directed tension toward tonic",
                pattern: &["V", "vii°"],
            },
            TheoryEntry {
                id: "secondary_dominant",
                name: "secondary dominant",
                description: "temporary dominant of a non-tonic goal",
                pattern: &["V/x", "x"],
            },
            TheoryEntry {
                id: "submediant",
                name: "submediant function",
                description: "relative-minor substitute and deceptive-resolution goal",
                pattern: &["vi"],
            },
        ],
        set_classes: vec![
            TheoryEntry {
                id: "016",
                name: "atonal trichord 016",
                description: "semitone plus tritone set class",
                pattern: &["0", "1", "6"],
            },
            TheoryEntry {
                id: "037",
                name: "minor/major triad set class",
                description: "common triadic pitch-class set",
                pattern: &["0", "3", "7"],
            },
            TheoryEntry {
                id: "0257",
                name: "quartal tetrachord",
                description: "stacked fourth-related set",
                pattern: &["0", "2", "5", "7"],
            },
            TheoryEntry {
                id: "all_interval_tetrachord",
                name: "all-interval tetrachord",
                description: "four-note set containing every interval class",
                pattern: &["0", "1", "4", "6"],
            },
        ],
        tuning_systems: vec![
            TheoryEntry {
                id: "equal_temperament_12",
                name: "12-tone equal temperament",
                description: "octave divided into twelve equal semitone steps",
                pattern: &["12-EDO"],
            },
            TheoryEntry {
                id: "just_intonation",
                name: "just intonation",
                description: "intervals tuned by small integer frequency ratios",
                pattern: &["3:2", "5:4", "6:5"],
            },
            TheoryEntry {
                id: "pythagorean",
                name: "Pythagorean tuning",
                description: "tuning generated from stacked pure fifths",
                pattern: &["3:2_cycle"],
            },
            TheoryEntry {
                id: "quarter_comma_meantone",
                name: "quarter-comma meantone",
                description: "temperament favoring pure major thirds",
                pattern: &["meantone", "5:4"],
            },
        ],
        world_traditions: vec![
            TheoryEntry {
                id: "hindustani_raga",
                name: "Hindustani raga",
                description: "melodic framework with ascent, descent, and emphasized tones",
                pattern: &["aroha", "avaroha", "vadi", "samvadi"],
            },
            TheoryEntry {
                id: "maqam",
                name: "Arabic maqam",
                description: "modal system built from jins and characteristic melodic behavior",
                pattern: &["jins", "sayr"],
            },
            TheoryEntry {
                id: "gamelan_slendro",
                name: "gamelan slendro",
                description: "five-tone Indonesian tuning and ensemble tradition",
                pattern: &["five_tones", "cyclic_colotomic_form"],
            },
            TheoryEntry {
                id: "west_african_timeline",
                name: "West African timeline pattern",
                description: "cyclic reference rhythm organizing ensemble parts",
                pattern: &["cycle", "timeline", "cross_rhythm"],
            },
        ],
        style_eras: vec![
            TheoryEntry {
                id: "renaissance",
                name: "Renaissance",
                description: "modal counterpoint and consonance treatment",
                pattern: &["modes", "imitative_counterpoint"],
            },
            TheoryEntry {
                id: "baroque",
                name: "Baroque",
                description: "functional harmony, basso continuo, sequences",
                pattern: &["functional_harmony", "counterpoint"],
            },
            TheoryEntry {
                id: "classical",
                name: "Classical",
                description: "periodic phrasing and tonal syntax",
                pattern: &["tonal_harmony", "periodic_phrase"],
            },
            TheoryEntry {
                id: "romantic",
                name: "Romantic",
                description: "chromatic harmony and expanded form",
                pattern: &["chromaticism", "expanded_tonality"],
            },
            TheoryEntry {
                id: "jazz",
                name: "Jazz",
                description: "extended harmony and swing vocabulary",
                pattern: &["seventh_chords", "extensions", "ii_V_I"],
            },
            TheoryEntry {
                id: "popular",
                name: "Popular",
                description: "loop-based harmonic and rhythmic idioms",
                pattern: &["diatonic_loops", "backbeat"],
            },
        ],
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CoreError {
    #[error("invalid pitch class `{0}`")]
    InvalidPitchClass(String),
    #[error("invalid pitch literal `{0}`")]
    InvalidPitchLiteral(String),
    #[error("invalid octave {0}; supported range is -1..=9")]
    InvalidOctave(i8),
    #[error("invalid MIDI note {0}; supported range is 0..=127")]
    InvalidMidiNumber(i16),
    #[error("duration denominator must not be zero")]
    ZeroDurationDenominator,
    #[error("duration must be positive")]
    NonPositiveDuration,
    #[error("invalid duration literal `{0}`")]
    InvalidDurationLiteral(String),
    #[error("chord must contain at least one pitch")]
    EmptyChord,
    #[error("invalid theory domain `{0}`")]
    InvalidTheoryDomain(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SourceId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    pub source_id: SourceId,
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
}

impl Span {
    pub const fn point(line: usize, column: usize) -> Self {
        Self {
            source_id: SourceId(0),
            start: 0,
            end: 0,
            line,
            column,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spanned<T> {
    pub value: T,
    pub span: Span,
}

impl<T> Spanned<T> {
    pub const fn new(value: T, span: Span) -> Self {
        Self { value, span }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticLabel {
    pub span: Span,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticRelated {
    pub span: Span,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: String,
    pub severity: Severity,
    pub message: String,
    pub line: usize,
    pub column: usize,
    pub span: Option<Span>,
    pub labels: Vec<DiagnosticLabel>,
    pub related: Vec<DiagnosticRelated>,
    pub rule: Option<String>,
    pub style: Option<String>,
    pub help: Option<String>,
}

impl Diagnostic {
    pub fn error(
        code: impl Into<String>,
        message: impl Into<String>,
        line: usize,
        column: usize,
    ) -> Self {
        Self {
            code: code.into(),
            severity: Severity::Error,
            message: message.into(),
            line,
            column,
            span: Some(Span::point(line, column)),
            labels: Vec::new(),
            related: Vec::new(),
            rule: None,
            style: None,
            help: None,
        }
    }

    pub fn warning(
        code: impl Into<String>,
        message: impl Into<String>,
        line: usize,
        column: usize,
    ) -> Self {
        let mut diagnostic = Self::error(code, message, line, column);
        diagnostic.severity = Severity::Warning;
        diagnostic
    }

    pub fn with_severity(mut self, severity: Severity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_span(mut self, span: Span) -> Self {
        self.line = span.line;
        self.column = span.column;
        self.span = Some(span);
        self
    }

    pub fn with_label(mut self, span: Span, message: impl Into<String>) -> Self {
        self.labels.push(DiagnosticLabel {
            span,
            message: message.into(),
        });
        self
    }

    pub fn with_related(mut self, span: Span, message: impl Into<String>) -> Self {
        self.related.push(DiagnosticRelated {
            span,
            message: message.into(),
        });
        self
    }

    pub fn with_rule(mut self, rule: impl Into<String>) -> Self {
        self.rule = Some(rule.into());
        self
    }

    pub fn with_style(mut self, style: impl Into<String>) -> Self {
        self.style = Some(style.into());
        self
    }

    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}[{}]: {}", self.severity, self.code, self.message)?;
        writeln!(f, "  at {}:{}", self.line, self.column)?;
        if let Some(style) = &self.style {
            writeln!(f, "  style: {style}")?;
        }
        if let Some(rule) = &self.rule {
            writeln!(f, "  rule: {rule}")?;
        }
        if let Some(help) = &self.help {
            writeln!(f, "  help: {help}")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MusicType {
    Int,
    Bool,
    Pitch,
    Interval,
    Duration,
    Chord,
    String,
    Unit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleSeverity {
    Error,
    Warning,
    Off,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Error => f.write_str("error"),
            Self::Warning => f.write_str("warning"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PitchClass {
    C,
    Cs,
    D,
    Ds,
    E,
    F,
    Fs,
    G,
    Gs,
    A,
    As,
    B,
}

impl PitchClass {
    pub const fn semitone(self) -> i16 {
        match self {
            Self::C => 0,
            Self::Cs => 1,
            Self::D => 2,
            Self::Ds => 3,
            Self::E => 4,
            Self::F => 5,
            Self::Fs => 6,
            Self::G => 7,
            Self::Gs => 8,
            Self::A => 9,
            Self::As => 10,
            Self::B => 11,
        }
    }

    pub const fn from_semitone(semitone: i16) -> Self {
        match semitone.rem_euclid(12) {
            0 => Self::C,
            1 => Self::Cs,
            2 => Self::D,
            3 => Self::Ds,
            4 => Self::E,
            5 => Self::F,
            6 => Self::Fs,
            7 => Self::G,
            8 => Self::Gs,
            9 => Self::A,
            10 => Self::As,
            _ => Self::B,
        }
    }
}

impl FromStr for PitchClass {
    type Err = CoreError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "C" => Ok(Self::C),
            "C#" | "Db" => Ok(Self::Cs),
            "D" => Ok(Self::D),
            "D#" | "Eb" => Ok(Self::Ds),
            "E" | "Fb" => Ok(Self::E),
            "F" | "E#" => Ok(Self::F),
            "F#" | "Gb" => Ok(Self::Fs),
            "G" => Ok(Self::G),
            "G#" | "Ab" => Ok(Self::Gs),
            "A" => Ok(Self::A),
            "A#" | "Bb" => Ok(Self::As),
            "B" | "Cb" => Ok(Self::B),
            _ => Err(CoreError::InvalidPitchClass(value.to_string())),
        }
    }
}

impl fmt::Display for PitchClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::C => "C",
            Self::Cs => "C#",
            Self::D => "D",
            Self::Ds => "D#",
            Self::E => "E",
            Self::F => "F",
            Self::Fs => "F#",
            Self::G => "G",
            Self::Gs => "G#",
            Self::A => "A",
            Self::As => "A#",
            Self::B => "B",
        };
        f.write_str(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Pitch {
    class: PitchClass,
    octave: i8,
}

impl Pitch {
    pub fn new(class: PitchClass, octave: i8) -> Result<Self, CoreError> {
        let pitch = Self { class, octave };
        pitch.midi_number()?;
        Ok(pitch)
    }

    pub fn from_midi_number(midi_number: i16) -> Result<Self, CoreError> {
        if !(0..=127).contains(&midi_number) {
            return Err(CoreError::InvalidMidiNumber(midi_number));
        }

        let class = PitchClass::from_semitone(midi_number);
        let octave = (midi_number / 12) as i8 - 1;
        Ok(Self { class, octave })
    }

    pub const fn class(self) -> PitchClass {
        self.class
    }

    pub const fn octave(self) -> i8 {
        self.octave
    }

    pub fn midi_number(self) -> Result<u8, CoreError> {
        if !(-1..=9).contains(&self.octave) {
            return Err(CoreError::InvalidOctave(self.octave));
        }

        let number = (i16::from(self.octave) + 1) * 12 + self.class.semitone();
        if !(0..=127).contains(&number) {
            return Err(CoreError::InvalidMidiNumber(number));
        }

        Ok(number as u8)
    }

    pub fn transpose(self, interval: Interval) -> Result<Self, CoreError> {
        let midi_number = i16::from(self.midi_number()?) + interval.semitones();
        Self::from_midi_number(midi_number)
    }
}

impl FromStr for Pitch {
    type Err = CoreError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let split_at = value
            .char_indices()
            .find_map(|(index, ch)| (ch.is_ascii_digit() || ch == '-').then_some(index))
            .ok_or_else(|| CoreError::InvalidPitchLiteral(value.to_string()))?;
        let (class, octave) = value.split_at(split_at);
        let octave = octave
            .parse::<i8>()
            .map_err(|_| CoreError::InvalidPitchLiteral(value.to_string()))?;
        Self::new(class.parse()?, octave)
    }
}

impl fmt::Display for Pitch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.class, self.octave)
    }
}

impl Add<Interval> for Pitch {
    type Output = Result<Pitch, CoreError>;

    fn add(self, rhs: Interval) -> Self::Output {
        self.transpose(rhs)
    }
}

impl Sub<Interval> for Pitch {
    type Output = Result<Pitch, CoreError>;

    fn sub(self, rhs: Interval) -> Self::Output {
        self.transpose(-rhs)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Interval {
    semitones: i16,
}

impl Interval {
    pub const fn new(semitones: i16) -> Self {
        Self { semitones }
    }

    pub const fn minor_third() -> Self {
        Self { semitones: 3 }
    }

    pub const fn major_third() -> Self {
        Self { semitones: 4 }
    }

    pub const fn perfect_fifth() -> Self {
        Self { semitones: 7 }
    }

    pub const fn semitones(self) -> i16 {
        self.semitones
    }
}

impl FromStr for Interval {
    type Err = CoreError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "m2" => Ok(Self::new(1)),
            "M2" => Ok(Self::new(2)),
            "m3" => Ok(Self::minor_third()),
            "M3" => Ok(Self::major_third()),
            "P4" => Ok(Self::new(5)),
            "TT" => Ok(Self::new(6)),
            "P5" => Ok(Self::perfect_fifth()),
            "m6" => Ok(Self::new(8)),
            "M6" => Ok(Self::new(9)),
            "m7" => Ok(Self::new(10)),
            "M7" => Ok(Self::new(11)),
            "P8" => Ok(Self::new(12)),
            _ => Err(CoreError::InvalidPitchLiteral(value.to_string())),
        }
    }
}

impl std::ops::Neg for Interval {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self {
            semitones: -self.semitones,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Duration {
    numerator: u32,
    denominator: u32,
}

impl Duration {
    pub fn new(numerator: u32, denominator: u32) -> Result<Self, CoreError> {
        if denominator == 0 {
            return Err(CoreError::ZeroDurationDenominator);
        }
        if numerator == 0 {
            return Err(CoreError::NonPositiveDuration);
        }

        let divisor = gcd(numerator, denominator);
        Ok(Self {
            numerator: numerator / divisor,
            denominator: denominator / divisor,
        })
    }

    pub const fn numerator(self) -> u32 {
        self.numerator
    }

    pub const fn denominator(self) -> u32 {
        self.denominator
    }

    pub fn ticks(self, ticks_per_quarter: u32) -> u32 {
        ticks_per_quarter * 4 * self.numerator / self.denominator
    }
}

impl FromStr for Duration {
    type Err = CoreError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let Some((numerator, denominator)) = value.split_once('/') else {
            return Err(CoreError::InvalidDurationLiteral(value.to_string()));
        };
        Self::new(
            numerator
                .parse()
                .map_err(|_| CoreError::InvalidDurationLiteral(value.to_string()))?,
            denominator
                .parse()
                .map_err(|_| CoreError::InvalidDurationLiteral(value.to_string()))?,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pitch: Pitch,
    duration: Duration,
    velocity: u8,
}

impl Note {
    pub const fn new(pitch: Pitch, duration: Duration) -> Self {
        Self {
            pitch,
            duration,
            velocity: 80,
        }
    }

    pub const fn with_velocity(pitch: Pitch, duration: Duration, velocity: u8) -> Self {
        Self {
            pitch,
            duration,
            velocity,
        }
    }

    pub const fn pitch(&self) -> Pitch {
        self.pitch
    }

    pub const fn duration(&self) -> Duration {
        self.duration
    }

    pub const fn velocity(&self) -> u8 {
        self.velocity
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chord {
    pitches: Vec<Pitch>,
    duration: Duration,
}

impl Chord {
    pub fn new(pitches: Vec<Pitch>, duration: Duration) -> Result<Self, CoreError> {
        if pitches.is_empty() {
            return Err(CoreError::EmptyChord);
        }

        Ok(Self { pitches, duration })
    }

    pub fn pitches(&self) -> &[Pitch] {
        &self.pitches
    }

    pub const fn duration(&self) -> Duration {
        self.duration
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoiceEvent {
    Note(Note),
    Chord(Chord),
}

impl VoiceEvent {
    pub fn duration(&self) -> Duration {
        match self {
            Self::Note(note) => note.duration(),
            Self::Chord(chord) => chord.duration(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Voice {
    name: String,
    events: Vec<VoiceEvent>,
}

impl Voice {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            events: Vec::new(),
        }
    }

    pub fn push(&mut self, event: VoiceEvent) {
        self.events.push(event);
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn events(&self) -> &[VoiceEvent] {
        &self.events
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Score {
    title: String,
    voices: Vec<Voice>,
}

impl Score {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            voices: Vec::new(),
        }
    }

    pub fn push_voice(&mut self, voice: Voice) {
        self.voices.push(voice);
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn voices(&self) -> &[Voice] {
        &self.voices
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScoreIr {
    pub title: String,
    pub composer: Option<String>,
    pub ticks_per_quarter: u32,
    pub tempo_bpm: u16,
    pub meter: Option<Meter>,
    pub key: Option<KeySignature>,
    pub tracks: Vec<TrackIr>,
    pub markers: Vec<MarkerIr>,
    pub tempo_changes: Vec<TempoChangeIr>,
    pub meter_changes: Vec<MeterChangeIr>,
    pub key_changes: Vec<KeyChangeIr>,
    pub overrides: Vec<OverrideTrace>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TempoChangeIr {
    pub bpm: u16,
    pub tick: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeterChangeIr {
    pub meter: Meter,
    pub tick: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyChangeIr {
    pub key: KeySignature,
    pub tick: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkerIr {
    pub label: String,
    pub tick: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeySignature {
    pub fifths: i8,
    pub is_minor: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Meter {
    pub numerator: u8,
    pub denominator: u8,
}

impl Default for Meter {
    fn default() -> Self {
        Self {
            numerator: 4,
            denominator: 4,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackIr {
    pub name: String,
    pub channel: u8,
    pub program: Option<u8>,
    pub events: Vec<NoteEventIr>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteEventIr {
    pub pitch: Pitch,
    pub start_tick: u32,
    pub duration_ticks: u32,
    pub velocity: u8,
    pub articulation: Option<String>,
    pub source_span: Option<Span>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverrideTrace {
    pub rule: String,
    pub reason: Option<String>,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyleContext {
    pub name: String,
    pub allowed_pitch_classes: Option<BTreeSet<PitchClass>>,
    pub chord_vocab: Vec<Vec<PitchClass>>,
    pub chord_quality_vocab: Vec<String>,
    pub set_class_vocab: Vec<String>,
    pub rhythm_vocab: Vec<Duration>,
    pub rhythm_concepts: Vec<String>,
    pub dynamic_vocab: Vec<String>,
    pub articulation_vocab: Vec<String>,
    pub ornaments: Vec<String>,
    pub non_chord_tones: Vec<String>,
    pub tuning_systems: Vec<String>,
    pub world_traditions: Vec<String>,
    pub historical_eras: Vec<String>,
    pub harmonic_functions: Vec<String>,
    pub max_melodic_leap: Option<Interval>,
    pub max_voice_spacing: Option<Interval>,
    pub contrapuntal_motion: Vec<String>,
    pub cadences: Vec<String>,
    pub harmonic_progression: Vec<String>,
    pub texture: Option<String>,
    pub form: Option<String>,
    pub meter: Option<Meter>,
    pub meter_catalog: Vec<String>,
    pub tempo_range: Option<(u16, u16)>,
    pub instrument_ranges: Vec<InstrumentRange>,
    pub theory: Vec<TheoryReference>,
    pub custom_theory: Vec<CustomTheoryDomain>,
    pub custom_rules: Vec<CustomStyleRule>,
    pub rule_severity: BTreeMap<String, RuleSeverity>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomStyleRule {
    pub id: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TheoryReference {
    pub domain: String,
    pub entry_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomTheoryDomain {
    pub name: String,
    pub entries: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstrumentRange {
    pub program: u8,
    pub low: Pitch,
    pub high: Pitch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StyleDescriptor {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
}

pub const BUILT_IN_STYLES: &[StyleDescriptor] = &[
    StyleDescriptor {
        id: "Classical",
        name: "Classical common-practice",
        description: "C-major common-practice checks for scale, triads, meter, tempo, rhythm, and voice-leading defaults",
    },
    StyleDescriptor {
        id: "Modal",
        name: "Modal open texture",
        description: "Dorian-centered modal writing with flexible meter and sparse harmonic constraints",
    },
    StyleDescriptor {
        id: "Jazz",
        name: "Jazz lead-sheet",
        description: "Extended chromatic pitch set with seventh-chord vocabulary, syncopated rhythms, and relaxed motion rules",
    },
    StyleDescriptor {
        id: "Minimalist",
        name: "Minimalist pulse",
        description: "Constrained pitch vocabulary, steady rhythmic cells, and narrow melodic motion for pattern-based music",
    },
];

impl StyleContext {
    pub fn core() -> Self {
        Self {
            name: "Core".to_string(),
            allowed_pitch_classes: None,
            chord_vocab: Vec::new(),
            chord_quality_vocab: Vec::new(),
            set_class_vocab: Vec::new(),
            rhythm_vocab: Vec::new(),
            rhythm_concepts: Vec::new(),
            dynamic_vocab: Vec::new(),
            articulation_vocab: Vec::new(),
            ornaments: Vec::new(),
            non_chord_tones: Vec::new(),
            tuning_systems: Vec::new(),
            world_traditions: Vec::new(),
            historical_eras: Vec::new(),
            harmonic_functions: Vec::new(),
            max_melodic_leap: None,
            max_voice_spacing: None,
            contrapuntal_motion: Vec::new(),
            cadences: Vec::new(),
            harmonic_progression: Vec::new(),
            texture: None,
            form: None,
            meter: None,
            meter_catalog: Vec::new(),
            tempo_range: None,
            instrument_ranges: Vec::new(),
            theory: Vec::new(),
            custom_theory: Vec::new(),
            custom_rules: Vec::new(),
            rule_severity: BTreeMap::new(),
        }
    }

    pub fn classical_c_major() -> Self {
        let mut style = Self::core();
        style.name = "Classical".to_string();
        style.allowed_pitch_classes = Some(BTreeSet::from([
            PitchClass::C,
            PitchClass::D,
            PitchClass::E,
            PitchClass::F,
            PitchClass::G,
            PitchClass::A,
            PitchClass::B,
        ]));
        style.chord_vocab = vec![
            vec![PitchClass::C, PitchClass::E, PitchClass::G],
            vec![PitchClass::D, PitchClass::F, PitchClass::A],
            vec![PitchClass::E, PitchClass::G, PitchClass::B],
            vec![PitchClass::F, PitchClass::A, PitchClass::C],
            vec![PitchClass::G, PitchClass::B, PitchClass::D],
            vec![PitchClass::A, PitchClass::C, PitchClass::E],
        ];
        style.rhythm_vocab = vec![
            Duration::new(1, 1).expect("valid duration"),
            Duration::new(1, 2).expect("valid duration"),
            Duration::new(1, 4).expect("valid duration"),
            Duration::new(1, 8).expect("valid duration"),
            Duration::new(1, 16).expect("valid duration"),
        ];
        style.max_melodic_leap = Some(Interval::new(12));
        style.max_voice_spacing = Some(Interval::new(12));
        style.contrapuntal_motion = vec![
            "contrary".to_string(),
            "oblique".to_string(),
            "similar".to_string(),
        ];
        style.meter = Some(Meter::default());
        style.tempo_range = Some((40, 208));
        style
    }

    pub fn modal() -> Self {
        let mut style = Self::core();
        style.name = "Modal".to_string();
        style.allowed_pitch_classes = Some(BTreeSet::from([
            PitchClass::C,
            PitchClass::D,
            PitchClass::E,
            PitchClass::F,
            PitchClass::G,
            PitchClass::A,
            PitchClass::B,
        ]));
        style.rhythm_vocab = vec![
            Duration::new(1, 1).expect("valid duration"),
            Duration::new(1, 2).expect("valid duration"),
            Duration::new(1, 4).expect("valid duration"),
            Duration::new(1, 8).expect("valid duration"),
        ];
        style.max_melodic_leap = Some(Interval::new(10));
        style.texture = Some("monophony".to_string());
        style.tempo_range = Some((48, 144));
        style
    }

    pub fn jazz() -> Self {
        let mut style = Self::core();
        style.name = "Jazz".to_string();
        style.allowed_pitch_classes = Some(BTreeSet::from([
            PitchClass::C,
            PitchClass::Cs,
            PitchClass::D,
            PitchClass::Ds,
            PitchClass::E,
            PitchClass::F,
            PitchClass::Fs,
            PitchClass::G,
            PitchClass::Gs,
            PitchClass::A,
            PitchClass::As,
            PitchClass::B,
        ]));
        style.chord_vocab = vec![
            vec![PitchClass::C, PitchClass::E, PitchClass::G, PitchClass::B],
            vec![PitchClass::D, PitchClass::F, PitchClass::A, PitchClass::C],
            vec![PitchClass::E, PitchClass::G, PitchClass::B, PitchClass::D],
            vec![PitchClass::F, PitchClass::A, PitchClass::C, PitchClass::E],
            vec![PitchClass::G, PitchClass::B, PitchClass::D, PitchClass::F],
            vec![PitchClass::A, PitchClass::C, PitchClass::E, PitchClass::G],
        ];
        style.rhythm_vocab = vec![
            Duration::new(1, 2).expect("valid duration"),
            Duration::new(1, 4).expect("valid duration"),
            Duration::new(1, 8).expect("valid duration"),
            Duration::new(1, 16).expect("valid duration"),
        ];
        style.tempo_range = Some((60, 260));
        style
            .rule_severity
            .insert("parallel_fifths".to_string(), RuleSeverity::Warning);
        style
            .rule_severity
            .insert("voice_crossing".to_string(), RuleSeverity::Warning);
        style
    }

    pub fn minimalist() -> Self {
        let mut style = Self::core();
        style.name = "Minimalist".to_string();
        style.allowed_pitch_classes = Some(BTreeSet::from([
            PitchClass::C,
            PitchClass::E,
            PitchClass::G,
        ]));
        style.rhythm_vocab = vec![
            Duration::new(1, 4).expect("valid duration"),
            Duration::new(1, 8).expect("valid duration"),
        ];
        style.max_melodic_leap = Some(Interval::new(7));
        style.texture = Some("monophony".to_string());
        style.tempo_range = Some((80, 180));
        style
    }

    pub fn named(name: &str) -> Self {
        match name {
            "Classical" => Self::classical_c_major(),
            "Modal" => Self::modal(),
            "Jazz" => Self::jazz(),
            "Minimalist" => Self::minimalist(),
            _ => Self::core(),
        }
    }

    pub fn built_in(name: &str) -> Option<Self> {
        BUILT_IN_STYLES
            .iter()
            .any(|style| style.id == name)
            .then(|| Self::named(name))
    }

    pub fn allows_pitch(&self, pitch: Pitch) -> bool {
        self.allowed_pitch_classes
            .as_ref()
            .is_none_or(|classes| classes.contains(&pitch.class()))
    }

    pub fn rule_severity(&self, rule: &str) -> RuleSeverity {
        self.rule_severity
            .get(rule)
            .copied()
            .unwrap_or(RuleSeverity::Error)
    }
}

const fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        let remainder = a % b;
        a = b;
        b = remainder;
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theory_catalog_covers_major_domains() {
        let catalog = theory_catalog();

        assert!(catalog.intervals.iter().any(|entry| entry.id == "P5"));
        assert!(catalog.scales.iter().any(|entry| entry.id == "blues"));
        assert!(catalog.modes.iter().any(|entry| entry.id == "dorian"));
        assert!(catalog
            .chord_qualities
            .iter()
            .any(|entry| entry.id == "dominant7"));
        assert!(catalog.cadences.iter().any(|entry| entry.id == "deceptive"));
        assert!(catalog.meters.iter().any(|entry| entry.id == "6/8"));
        assert!(catalog.rhythms.iter().any(|entry| entry.id == "hemiola"));
        assert!(catalog.forms.iter().any(|entry| entry.id == "sonata"));
        assert!(catalog.textures.iter().any(|entry| entry.id == "polyphony"));
        assert!(catalog.ornaments.iter().any(|entry| entry.id == "trill"));
        assert!(catalog.ornaments.iter().any(|entry| entry.id == "staccato"));
        assert!(catalog
            .contrapuntal_motions
            .iter()
            .any(|entry| entry.id == "contrary"));
        assert!(catalog
            .non_chord_tones
            .iter()
            .any(|entry| entry.id == "suspension"));
        assert!(catalog
            .harmonic_functions
            .iter()
            .any(|entry| entry.id == "dominant"));
        assert!(catalog
            .set_classes
            .iter()
            .any(|entry| entry.id == "all_interval_tetrachord"));
        assert!(catalog
            .tuning_systems
            .iter()
            .any(|entry| entry.id == "just_intonation"));
        assert!(catalog
            .world_traditions
            .iter()
            .any(|entry| entry.id == "maqam"));
        assert!(catalog.style_eras.iter().any(|entry| entry.id == "jazz"));
    }

    #[test]
    fn theory_catalog_is_queryable_by_domain_and_id() {
        let catalog = theory_catalog();

        assert_eq!(TheoryCatalog::domains().len(), 18);
        assert!(catalog
            .entries(TheoryDomain::Scales)
            .iter()
            .any(|entry| entry.id == "major_pentatonic"));
        assert!(catalog
            .entries(TheoryDomain::Dynamics)
            .iter()
            .any(|entry| entry.id == "mf" && entry.pattern == ["80"]));
        assert!(catalog
            .entries(TheoryDomain::HarmonicFunctions)
            .iter()
            .any(|entry| entry.id == "secondary_dominant"));

        let (domain, entry) = catalog.find("maqam").unwrap();
        assert_eq!(domain, TheoryDomain::WorldTraditions);
        assert_eq!(entry.name, "Arabic maqam");
        assert_eq!(
            "scales".parse::<TheoryDomain>().unwrap(),
            TheoryDomain::Scales
        );
        assert_eq!(
            "harmonic_functions".parse::<TheoryDomain>().unwrap(),
            TheoryDomain::HarmonicFunctions
        );
    }

    #[test]
    fn diagnostic_error_keeps_legacy_location_and_span() {
        let diagnostic = Diagnostic::error("ML_TEST", "test", 3, 7);

        assert_eq!(diagnostic.line, 3);
        assert_eq!(diagnostic.column, 7);
        assert_eq!(diagnostic.span.unwrap().line, 3);
        assert_eq!(diagnostic.span.unwrap().column, 7);
    }

    #[test]
    fn parses_pitch_literal() {
        assert_eq!(
            "C4".parse::<Pitch>().unwrap(),
            Pitch::new(PitchClass::C, 4).unwrap()
        );
        assert_eq!(
            "Bb3".parse::<Pitch>().unwrap(),
            Pitch::new(PitchClass::As, 3).unwrap()
        );
    }

    #[test]
    fn transposes_c4_by_major_third_to_e4() {
        let c4 = Pitch::new(PitchClass::C, 4).unwrap();
        let e4 = c4 + Interval::major_third();

        assert_eq!(e4.unwrap(), Pitch::new(PitchClass::E, 4).unwrap());
    }

    #[test]
    fn transposes_e4_down_by_minor_third_to_cs4() {
        let e4 = Pitch::new(PitchClass::E, 4).unwrap();
        let cs4 = e4 - Interval::minor_third();

        assert_eq!(cs4.unwrap(), Pitch::new(PitchClass::Cs, 4).unwrap());
    }

    #[test]
    fn rejects_pitch_outside_midi_range() {
        assert_eq!(
            Pitch::new(PitchClass::B, 9).unwrap_err(),
            CoreError::InvalidMidiNumber(131)
        );
    }

    #[test]
    fn rejects_zero_duration_denominator() {
        assert_eq!(
            Duration::new(1, 0).unwrap_err(),
            CoreError::ZeroDurationDenominator
        );
    }

    #[test]
    fn rejects_zero_duration_numerator() {
        assert_eq!(
            Duration::new(0, 4).unwrap_err(),
            CoreError::NonPositiveDuration
        );
    }

    #[test]
    fn normalizes_duration() {
        let duration = Duration::new(2, 8).unwrap();

        assert_eq!(duration.numerator(), 1);
        assert_eq!(duration.denominator(), 4);
    }

    #[test]
    fn rejects_empty_chord() {
        let duration = Duration::new(1, 4).unwrap();

        assert_eq!(Chord::new(Vec::new(), duration), Err(CoreError::EmptyChord));
    }
}
