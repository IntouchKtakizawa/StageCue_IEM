#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::Serialize;
use stagecue_audio_engine::AudioService;
use stagecue_project_model::{preflight_project, PreflightContext, SongProject, ValidationReport};
use stagecue_project_model::{
    MeterChange, MusicalPosition, SongSection, TempoChange, TempoTransition,
};
use stagecue_timing_engine::Timeline;
use std::path::PathBuf;

#[tauri::command]
fn load_project(path: PathBuf) -> Result<SongProject, String> {
    stagecue_project_store::load(path).map_err(|error| error.to_string())
}

#[tauri::command]
fn save_project(path: PathBuf, project: SongProject) -> Result<(), String> {
    stagecue_project_store::save_atomic(path, &project).map_err(|error| error.to_string())
}

#[tauri::command]
fn preflight(
    path: PathBuf,
    project: SongProject,
    output_channels: u16,
    sample_rate: u32,
) -> ValidationReport {
    preflight_project(
        &project,
        &path,
        &PreflightContext {
            device_available: true,
            available_output_channels: output_channels,
            device_sample_rate: sample_rate,
        },
    )
}

#[tauri::command]
fn audio_play(audio: tauri::State<'_, AudioService>) -> Result<(), String> {
    audio.play().map_err(|error| error.to_string())
}

#[tauri::command]
fn audio_stop(audio: tauri::State<'_, AudioService>) -> Result<(), String> {
    audio.stop().map_err(|error| error.to_string())
}

#[tauri::command]
fn audio_panic(audio: tauri::State<'_, AudioService>) -> Result<(), String> {
    audio.panic_mute().map_err(|error| error.to_string())
}

#[tauri::command]
fn audio_test_tone(audio: tauri::State<'_, AudioService>) -> Result<(), String> {
    audio.test_tone().map_err(|error| error.to_string())
}

#[tauri::command]
fn audio_status(audio: tauri::State<'_, AudioService>) -> (bool, bool) {
    (audio.callback_has_audio(), audio.device_failed())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PositionStatus {
    bar: i32,
    beat: u16,
    elapsed_seconds: f64,
    current_section: String,
    next_section: Option<String>,
    bars_remaining: u32,
    playing: bool,
}

#[tauri::command]
fn audio_position(
    audio: tauri::State<'_, AudioService>,
    bpm: f64,
    sections: Vec<SongSection>,
) -> Result<PositionStatus, String> {
    let rate = audio.sample_rate();
    let samples = audio.sample_clock();
    let timeline = Timeline::new(
        vec![MeterChange {
            position: MusicalPosition::new(1, 1, 0),
            numerator: 4,
            denominator: 4,
        }],
        vec![TempoChange {
            position: MusicalPosition::new(1, 1, 0),
            bpm,
            transition: TempoTransition::Instant,
        }],
    )
    .map_err(|error| error.to_string())?;
    let position = timeline
        .samples_to_position(samples, rate)
        .map_err(|error| error.to_string())?;
    let current_index = sections
        .iter()
        .enumerate()
        .rev()
        .find(|(_, section)| section.start.bar <= position.bar)
        .map(|(index, _)| index);
    let current = current_index.and_then(|index| sections.get(index));
    let next = current_index
        .and_then(|index| sections.get(index + 1))
        .or_else(|| sections.first().filter(|_| current.is_none()));
    let bars_remaining = current
        .map(|section| {
            (section.start.bar + section.length_bars as i32 - position.bar).max(0) as u32
        })
        .unwrap_or(0);
    Ok(PositionStatus {
        bar: position.bar,
        beat: position.beat,
        elapsed_seconds: samples as f64 / rate as f64,
        current_section: current
            .map(|section| section.name.clone())
            .unwrap_or_else(|| "Count-in".into()),
        next_section: next.map(|section| section.name.clone()),
        bars_remaining,
        playing: audio.is_playing(),
    })
}

fn main() {
    let audio = AudioService::start().expect("failed to start audio service");
    tauri::Builder::default()
        .manage(audio)
        .invoke_handler(tauri::generate_handler![
            load_project,
            save_project,
            preflight,
            audio_play,
            audio_stop,
            audio_panic,
            audio_test_tone,
            audio_status,
            audio_position
        ])
        .run(tauri::generate_context!())
        .expect("failed to run StageCue");
}
