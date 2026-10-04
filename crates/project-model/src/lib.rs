use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use thiserror::Error;

pub const CURRENT_SCHEMA_VERSION: u32 = 1;
pub const PPQ: u32 = 960;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicalPosition {
    /// One-based in files/UI. Zero and negative bars are count-in bars.
    pub bar: i32,
    /// One-based within the bar.
    pub beat: u16,
    /// Zero-based within the beat, in 960 PPQ quarter-note ticks.
    pub tick: u32,
}

impl MusicalPosition {
    pub const fn new(bar: i32, beat: u16, tick: u32) -> Self {
        Self { bar, beat, tick }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SongProject {
    pub schema_version: u32,
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spotify_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lyrics: Option<String>,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub audio_assets: Vec<AudioAsset>,
    pub tempo_map: Vec<TempoChange>,
    pub meter_map: Vec<MeterChange>,
    #[serde(default)]
    pub free_time_regions: Vec<FreeTimeRegion>,
    #[serde(default)]
    pub sections: Vec<SongSection>,
    #[serde(default)]
    pub events: Vec<TimelineEvent>,
    pub click: ClickConfiguration,
    #[serde(default)]
    pub buses: Vec<OutputBus>,
    #[serde(default)]
    pub routing: Vec<RouteAssignment>,
    pub performance: PerformanceSettings,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioAsset {
    pub id: String,
    pub name: String,
    pub path: String,
    pub role: AudioAssetRole,
    pub channels: u16,
    pub sample_rate: Option<u32>,
    pub offset_samples: i64,
    pub gain_db: f32,
    pub muted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AudioAssetRole {
    Backing,
    Stem,
    Reference,
    Cue,
    Click,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TempoChange {
    pub position: MusicalPosition,
    pub bpm: f64,
    pub transition: TempoTransition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TempoTransition {
    Instant,
    Linear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MeterChange {
    pub position: MusicalPosition,
    pub numerator: u16,
    pub denominator: u16,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FreeTimeRegion {
    pub id: String,
    pub start_seconds: f64,
    pub end_seconds: f64,
    #[serde(default = "default_true")]
    pub mute_click: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SongSection {
    pub id: String,
    pub name: String,
    pub start: MusicalPosition,
    pub length_bars: u32,
    pub color: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum TimelineEvent {
    VoiceCue(VoiceCueEvent),
    CountIn(CountInEvent),
    Marker(MarkerEvent),
    Midi(MidiEvent),
    Automation(AutomationEvent),
}

impl TimelineEvent {
    pub fn id(&self) -> &str {
        match self {
            Self::VoiceCue(v) => &v.id,
            Self::CountIn(v) => &v.id,
            Self::Marker(v) => &v.id,
            Self::Midi(v) => &v.id,
            Self::Automation(v) => &v.id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum EventTiming {
    Musical { position: MusicalPosition },
    Seconds { seconds: f64 },
    BarsBeforeSection { section_id: String, bars: u32 },
    BeatsBeforeSection { section_id: String, beats: u32 },
    SectionBoundary { section_id: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceCueEvent {
    pub id: String,
    pub timing: EventTiming,
    pub text: String,
    pub rendered_audio_asset_id: String,
    pub destination_bus_id: String,
    pub gain_db: f32,
    pub interrupt_policy: InterruptPolicy,
    #[serde(default)]
    pub pronunciation_overrides: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InterruptPolicy {
    Mix,
    DuckPrevious,
    SkipIfOverlapping,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CountInEvent {
    pub id: String,
    pub timing: EventTiming,
    pub count: u16,
    pub subdivision: ClickSubdivision,
    pub destination_bus_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkerEvent {
    pub id: String,
    pub timing: EventTiming,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MidiEvent {
    pub id: String,
    pub timing: EventTiming,
    pub data: Vec<u8>,
    pub port: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomationEvent {
    pub id: String,
    pub timing: EventTiming,
    pub target: String,
    pub value: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClickConfiguration {
    pub enabled: bool,
    pub gain_db: f32,
    pub subdivision: ClickSubdivision,
    pub accented_asset_id: Option<String>,
    pub unaccented_asset_id: Option<String>,
    #[serde(default)]
    pub patterns: Vec<ClickPattern>,
    #[serde(default)]
    pub enabled_section_ids: Vec<String>,
    #[serde(default = "default_true")]
    pub mute_in_free_time: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClickSubdivision {
    Quarter,
    Eighth,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClickPattern {
    pub numerator: u16,
    pub denominator: u16,
    pub steps: Vec<ClickStep>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClickStep {
    pub beat: f32,
    pub accent: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputBus {
    pub id: String,
    pub name: String,
    pub channel_count: u8,
    pub gain_db: f32,
    pub muted: bool,
    pub soloed: bool,
    pub hardware_channels: Vec<u16>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteAssignment {
    pub source_id: String,
    pub destination_bus_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceSettings {
    pub safety_fade_ms: u32,
    pub auto_advance_setlist: bool,
    pub stop_confirmation: bool,
    pub beginner_mode: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Setlist {
    pub schema_version: u32,
    pub id: String,
    pub title: String,
    pub entries: Vec<SetlistEntry>,
    #[serde(default)]
    pub auto_advance: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetlistEntry {
    pub id: String,
    pub project_path: String,
    pub title: String,
    #[serde(default)]
    pub notes: String,
    pub estimated_seconds: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SongTemplate {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub default_tempo: f64,
    pub meter: MeterChange,
    pub count_in_bars: u32,
    pub sections: Vec<TemplateSection>,
    pub cue_lead_bars: u32,
    pub click: ClickConfiguration,
    pub routing_suggestions: Vec<OutputBus>,
    pub preflight_requirements: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateSection {
    pub name: String,
    pub length_bars: u32,
    pub color: String,
    pub major: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreflightContext {
    pub device_available: bool,
    pub available_output_channels: u16,
    pub device_sample_rate: u32,
}

pub fn preflight_project(
    project: &SongProject,
    project_directory: &Path,
    context: &PreflightContext,
) -> ValidationReport {
    let mut report = validate_project(project);
    if !context.device_available {
        report.push(
            ValidationSeverity::Error,
            "device-unavailable",
            "routing",
            "selected audio device is unavailable",
        );
    }
    for (i, asset) in project.audio_assets.iter().enumerate() {
        let path = Path::new(&asset.path);
        let resolved = if path.is_absolute() {
            path.to_path_buf()
        } else {
            project_directory.join(path)
        };
        if !resolved.exists() {
            report.push(
                ValidationSeverity::Error,
                "missing-asset",
                format!("audioAssets[{i}].path"),
                format!("audio file does not exist: {}", resolved.display()),
            );
        }
        if let Some(rate) = asset.sample_rate {
            if rate != context.device_sample_rate {
                report.push(
                    ValidationSeverity::Warning,
                    "sample-rate-mismatch",
                    format!("audioAssets[{i}].sampleRate"),
                    format!(
                        "asset is {rate} Hz; output is {} Hz and requires offline resampling",
                        context.device_sample_rate
                    ),
                );
            }
        }
        if asset.muted && matches!(asset.role, AudioAssetRole::Backing | AudioAssetRole::Stem) {
            report.push(
                ValidationSeverity::Warning,
                "muted-performance-asset",
                format!("audioAssets[{i}].muted"),
                format!("{} is muted", asset.name),
            );
        }
    }
    for (i, bus) in project.buses.iter().enumerate() {
        if bus
            .hardware_channels
            .iter()
            .any(|c| *c > context.available_output_channels)
        {
            report.push(
                ValidationSeverity::Error,
                "unavailable-output",
                format!("buses[{i}].hardwareChannels"),
                "route requests a hardware output that is unavailable",
            );
        }
        if bus.gain_db > 0.0 {
            report.push(
                ValidationSeverity::Warning,
                "positive-bus-gain",
                format!("buses[{i}].gainDb"),
                "positive bus gain can clip the master output",
            );
        }
    }
    report
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ValidationSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationIssue {
    pub severity: ValidationSeverity,
    pub code: String,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationReport {
    pub issues: Vec<ValidationIssue>,
}

impl ValidationReport {
    pub fn is_valid(&self) -> bool {
        !self
            .issues
            .iter()
            .any(|i| i.severity == ValidationSeverity::Error)
    }
    pub fn errors(&self) -> impl Iterator<Item = &ValidationIssue> {
        self.issues
            .iter()
            .filter(|i| i.severity == ValidationSeverity::Error)
    }
    pub fn warnings(&self) -> impl Iterator<Item = &ValidationIssue> {
        self.issues
            .iter()
            .filter(|i| i.severity == ValidationSeverity::Warning)
    }
    fn push(
        &mut self,
        severity: ValidationSeverity,
        code: &str,
        path: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.issues.push(ValidationIssue {
            severity,
            code: code.into(),
            path: path.into(),
            message: message.into(),
        });
    }
}

pub fn validate_project(project: &SongProject) -> ValidationReport {
    let mut out = ValidationReport::default();
    if project.schema_version != CURRENT_SCHEMA_VERSION {
        out.push(
            ValidationSeverity::Error,
            "unsupported-schema",
            "schemaVersion",
            format!(
                "expected schema {}, got {}",
                CURRENT_SCHEMA_VERSION, project.schema_version
            ),
        );
    }
    if project.id.trim().is_empty() {
        out.push(
            ValidationSeverity::Error,
            "empty-id",
            "id",
            "project ID is required",
        );
    }
    if project.title.trim().is_empty() {
        out.push(
            ValidationSeverity::Error,
            "empty-title",
            "title",
            "title is required",
        );
    }

    let mut ids = HashSet::new();
    let mut register = |id: &str, path: String, out: &mut ValidationReport| {
        if id.trim().is_empty() {
            out.push(
                ValidationSeverity::Error,
                "empty-id",
                path,
                "ID is required",
            );
        } else if !ids.insert(id.to_owned()) {
            out.push(
                ValidationSeverity::Error,
                "duplicate-id",
                path,
                format!("duplicate ID {id}"),
            );
        }
    };
    for (i, a) in project.audio_assets.iter().enumerate() {
        register(&a.id, format!("audioAssets[{i}].id"), &mut out);
        if a.channels == 0 {
            out.push(
                ValidationSeverity::Error,
                "invalid-channels",
                format!("audioAssets[{i}].channels"),
                "channel count must be positive",
            );
        }
        if a.path.trim().is_empty() {
            out.push(
                ValidationSeverity::Error,
                "empty-path",
                format!("audioAssets[{i}].path"),
                "asset path is required",
            );
        }
    }
    for (i, s) in project.sections.iter().enumerate() {
        register(&s.id, format!("sections[{i}].id"), &mut out);
        validate_position(s.start, &format!("sections[{i}].start"), &mut out);
        if s.length_bars == 0 {
            out.push(
                ValidationSeverity::Error,
                "zero-section-length",
                format!("sections[{i}].lengthBars"),
                "section must contain at least one bar",
            );
        }
    }
    for (i, e) in project.events.iter().enumerate() {
        register(e.id(), format!("events[{i}].id"), &mut out);
    }
    for (i, b) in project.buses.iter().enumerate() {
        register(&b.id, format!("buses[{i}].id"), &mut out);
        if !matches!(b.channel_count, 1 | 2) {
            out.push(
                ValidationSeverity::Error,
                "invalid-bus-width",
                format!("buses[{i}].channelCount"),
                "bus must be mono or stereo",
            );
        }
        if b.hardware_channels.len() != b.channel_count as usize {
            out.push(
                ValidationSeverity::Error,
                "route-width-mismatch",
                format!("buses[{i}].hardwareChannels"),
                "hardware channel count must match bus width",
            );
        }
        if b.hardware_channels.contains(&0) {
            out.push(
                ValidationSeverity::Error,
                "invalid-hardware-channel",
                format!("buses[{i}].hardwareChannels"),
                "hardware channels are one-based",
            );
        }
    }
    if project.tempo_map.is_empty() {
        out.push(
            ValidationSeverity::Error,
            "missing-tempo",
            "tempoMap",
            "at least one tempo marker is required",
        );
    }
    for (i, t) in project.tempo_map.iter().enumerate() {
        validate_position(t.position, &format!("tempoMap[{i}].position"), &mut out);
        if !t.bpm.is_finite() || t.bpm <= 0.0 {
            out.push(
                ValidationSeverity::Error,
                "invalid-tempo",
                format!("tempoMap[{i}].bpm"),
                "BPM must be finite and positive",
            );
        }
        if t.transition == TempoTransition::Linear && i + 1 == project.tempo_map.len() {
            out.push(
                ValidationSeverity::Warning,
                "terminal-ramp",
                format!("tempoMap[{i}].transition"),
                "a final linear marker has no target and behaves as instant",
            );
        }
    }
    if project.meter_map.is_empty() {
        out.push(
            ValidationSeverity::Error,
            "missing-meter",
            "meterMap",
            "at least one meter marker is required",
        );
    }
    for (i, m) in project.meter_map.iter().enumerate() {
        validate_position(m.position, &format!("meterMap[{i}].position"), &mut out);
        if m.position.beat != 1 || m.position.tick != 0 {
            out.push(
                ValidationSeverity::Error,
                "meter-not-on-bar",
                format!("meterMap[{i}].position"),
                "meter changes must occur at a bar boundary",
            );
        }
        if m.numerator == 0 || !matches!(m.denominator, 1 | 2 | 4 | 8 | 16 | 32) {
            out.push(
                ValidationSeverity::Error,
                "invalid-meter",
                format!("meterMap[{i}]"),
                "meter must have a positive numerator and power-of-two denominator",
            );
        }
    }

    let asset_ids: HashSet<_> = project.audio_assets.iter().map(|a| a.id.as_str()).collect();
    let bus_ids: HashSet<_> = project.buses.iter().map(|b| b.id.as_str()).collect();
    let section_ids: HashSet<_> = project.sections.iter().map(|s| s.id.as_str()).collect();
    for (i, event) in project.events.iter().enumerate() {
        let (timing, bus, asset) = match event {
            TimelineEvent::VoiceCue(v) => (
                &v.timing,
                Some(v.destination_bus_id.as_str()),
                Some(v.rendered_audio_asset_id.as_str()),
            ),
            TimelineEvent::CountIn(v) => (&v.timing, Some(v.destination_bus_id.as_str()), None),
            TimelineEvent::Marker(v) => (&v.timing, None, None),
            TimelineEvent::Midi(v) => (&v.timing, None, None),
            TimelineEvent::Automation(v) => (&v.timing, None, None),
        };
        if let Some(id) = bus {
            if !bus_ids.contains(id) {
                out.push(
                    ValidationSeverity::Error,
                    "missing-bus",
                    format!("events[{i}].destinationBusId"),
                    format!("unknown bus {id}"),
                );
            }
        }
        if let Some(id) = asset {
            if !asset_ids.contains(id) {
                out.push(
                    ValidationSeverity::Error,
                    "missing-cue-audio",
                    format!("events[{i}].renderedAudioAssetId"),
                    format!("unknown asset {id}"),
                );
            }
        }
        match timing {
            EventTiming::Musical { position } => {
                validate_position(*position, &format!("events[{i}].timing.position"), &mut out)
            }
            EventTiming::Seconds { seconds } if !seconds.is_finite() => out.push(
                ValidationSeverity::Error,
                "invalid-time",
                format!("events[{i}].timing.seconds"),
                "seconds must be finite",
            ),
            EventTiming::BarsBeforeSection { section_id, .. }
            | EventTiming::BeatsBeforeSection { section_id, .. }
            | EventTiming::SectionBoundary { section_id }
                if !section_ids.contains(section_id.as_str()) =>
            {
                out.push(
                    ValidationSeverity::Error,
                    "missing-section",
                    format!("events[{i}].timing.sectionId"),
                    format!("unknown section {section_id}"),
                )
            }
            _ => {}
        }
    }
    for (i, route) in project.routing.iter().enumerate() {
        if !asset_ids.contains(route.source_id.as_str()) {
            out.push(
                ValidationSeverity::Error,
                "missing-route-source",
                format!("routing[{i}].sourceId"),
                "route source does not exist",
            );
        }
        if !bus_ids.contains(route.destination_bus_id.as_str()) {
            out.push(
                ValidationSeverity::Error,
                "missing-route-bus",
                format!("routing[{i}].destinationBusId"),
                "route destination does not exist",
            );
        }
    }
    out
}

fn validate_position(p: MusicalPosition, path: &str, out: &mut ValidationReport) {
    if p.beat == 0 {
        out.push(
            ValidationSeverity::Error,
            "invalid-beat",
            format!("{path}.beat"),
            "beats are one-based",
        );
    }
    if p.tick >= PPQ {
        out.push(
            ValidationSeverity::Error,
            "invalid-tick",
            format!("{path}.tick"),
            format!("tick must be below {PPQ}"),
        );
    }
}

#[derive(Debug, Error)]
pub enum MigrationError {
    #[error("project JSON must be an object")]
    NotAnObject,
    #[error("schemaVersion is missing or invalid")]
    MissingVersion,
    #[error("schema version {0} is newer than this application supports")]
    FutureVersion(u64),
    #[error("cannot deserialize migrated project: {0}")]
    Deserialize(#[from] serde_json::Error),
}

/// Migrates an in-memory copy. Callers must preserve the original file and save atomically.
pub fn migrate_value(mut value: Value) -> Result<SongProject, MigrationError> {
    let object = value.as_object_mut().ok_or(MigrationError::NotAnObject)?;
    let version = object
        .get("schemaVersion")
        .and_then(Value::as_u64)
        .ok_or(MigrationError::MissingVersion)?;
    if version > CURRENT_SCHEMA_VERSION as u64 {
        return Err(MigrationError::FutureVersion(version));
    }
    if version == 0 {
        object.insert("schemaVersion".into(), Value::from(1));
        object
            .entry("notes")
            .or_insert_with(|| Value::String(String::new()));
        object
            .entry("freeTimeRegions")
            .or_insert_with(|| Value::Array(vec![]));
    }
    Ok(serde_json::from_value(value)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> SongProject {
        serde_json::from_str(include_str!(
            "../../../fixtures/standard-rock.stagecue.json"
        ))
        .unwrap()
    }

    #[test]
    fn fixture_round_trips_and_validates() {
        let project = fixture();
        assert!(validate_project(&project).is_valid());
        let round_trip: SongProject =
            serde_json::from_str(&serde_json::to_string(&project).unwrap()).unwrap();
        assert_eq!(project, round_trip);
    }

    #[test]
    fn catches_invalid_route_and_duplicate_id() {
        let mut project = fixture();
        project.routing[0].destination_bus_id = "gone".into();
        let duplicate_id = project.sections[0].id.clone();
        project.sections[1].id = duplicate_id;
        let report = validate_project(&project);
        assert!(!report.is_valid());
        assert!(report.errors().any(|e| e.code == "missing-route-bus"));
        assert!(report.errors().any(|e| e.code == "duplicate-id"));
    }

    #[test]
    fn migrates_a_copy_of_v0() {
        let current = serde_json::to_value(fixture()).unwrap();
        let mut old = current.clone();
        old.as_object_mut()
            .unwrap()
            .insert("schemaVersion".into(), Value::from(0));
        old.as_object_mut().unwrap().remove("notes");
        old.as_object_mut().unwrap().remove("freeTimeRegions");
        let migrated = migrate_value(old.clone()).unwrap();
        assert_eq!(migrated.schema_version, 1);
        assert_eq!(old["schemaVersion"], 0);
    }
}
