//! Backend-neutral scheduling, WAV decoding and deterministic offline rendering.
#![forbid(unsafe_code)]

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam_queue::ArrayQueue;
use stagecue_project_model::{AudioAssetRole, MusicalPosition, OutputBus};
use stagecue_timing_engine::{Timeline, TimingError};
use std::{
    collections::HashMap,
    path::Path,
    process::Command,
    sync::{
        atomic::{AtomicBool, AtomicI64, Ordering},
        mpsc::{sync_channel, Receiver, SyncSender, TryRecvError},
        Arc,
    },
    thread,
    time::Duration,
};
use thiserror::Error;

/// Backend-neutral identity, allowing CPAL to be replaced by ASIO/JUCE later.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioDeviceId(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioDeviceInfo {
    pub id: AudioDeviceId,
    pub name: String,
    pub output_channels: u16,
    pub sample_rates: Vec<u32>,
}

/// Adapter seam for CPAL now and ASIO/JUCE later.
pub trait AudioBackend {
    fn output_devices(&self) -> Result<Vec<AudioDeviceInfo>, AudioError>;
    fn open(&mut self, device: &AudioDeviceId, config: StreamConfig) -> Result<(), AudioError>;
    fn stop(&mut self);
}

pub struct RealtimeControl {
    commands: Arc<ArrayQueue<TransportCommand>>,
    sample_clock: Arc<AtomicI64>,
    device_failed: Arc<AtomicBool>,
}

impl RealtimeControl {
    pub fn new(capacity: usize) -> Self {
        Self {
            commands: Arc::new(ArrayQueue::new(capacity.max(1))),
            sample_clock: Arc::new(AtomicI64::new(0)),
            device_failed: Arc::new(AtomicBool::new(false)),
        }
    }
    pub fn send(&self, command: TransportCommand) -> Result<(), TransportCommand> {
        self.commands.push(command)
    }
    pub fn sample_clock(&self) -> i64 {
        self.sample_clock.load(Ordering::Acquire)
    }
    pub fn device_failed(&self) -> bool {
        self.device_failed.load(Ordering::Acquire)
    }
}

impl Default for RealtimeControl {
    fn default() -> Self {
        Self::new(64)
    }
}

#[derive(Debug, Clone, Copy)]
enum ServiceCommand {
    Play,
    Stop,
    PanicMute,
    TestTone,
    Shutdown,
}

/// Owns CPAL on a dedicated thread. UI calls only enqueue bounded commands.
pub struct AudioService {
    sender: SyncSender<ServiceCommand>,
    callback_has_audio: Arc<AtomicBool>,
    device_failed: Arc<AtomicBool>,
    sample_clock: Arc<AtomicI64>,
    playing: Arc<AtomicBool>,
    sample_rate: Arc<AtomicI64>,
}

impl AudioService {
    pub fn start() -> Result<Self, AudioError> {
        let (sender, receiver) = sync_channel(32);
        let callback_has_audio = Arc::new(AtomicBool::new(false));
        let device_failed = Arc::new(AtomicBool::new(false));
        let has_audio = Arc::clone(&callback_has_audio);
        let failed = Arc::clone(&device_failed);
        let sample_clock = Arc::new(AtomicI64::new(0));
        let playing = Arc::new(AtomicBool::new(false));
        let sample_rate = Arc::new(AtomicI64::new(48_000));
        let worker_clock = Arc::clone(&sample_clock);
        let worker_playing = Arc::clone(&playing);
        let worker_rate = Arc::clone(&sample_rate);
        thread::Builder::new()
            .name("stagecue-audio".into())
            .spawn(move || {
                run_audio_service(
                    receiver,
                    has_audio,
                    failed,
                    worker_clock,
                    worker_playing,
                    worker_rate,
                )
            })
            .map_err(|error| AudioError::Device(error.to_string()))?;
        Ok(Self {
            sender,
            callback_has_audio,
            device_failed,
            sample_clock,
            playing,
            sample_rate,
        })
    }
    pub fn play(&self) -> Result<(), AudioError> {
        self.send(ServiceCommand::Play)
    }
    pub fn stop(&self) -> Result<(), AudioError> {
        self.send(ServiceCommand::Stop)
    }
    pub fn panic_mute(&self) -> Result<(), AudioError> {
        self.send(ServiceCommand::PanicMute)
    }
    pub fn test_tone(&self) -> Result<(), AudioError> {
        self.send(ServiceCommand::TestTone)
    }
    pub fn callback_has_audio(&self) -> bool {
        self.callback_has_audio.load(Ordering::Acquire)
    }
    pub fn device_failed(&self) -> bool {
        self.device_failed.load(Ordering::Acquire)
    }
    pub fn sample_clock(&self) -> i64 {
        self.sample_clock.load(Ordering::Acquire)
    }
    pub fn is_playing(&self) -> bool {
        self.playing.load(Ordering::Acquire)
    }
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate.load(Ordering::Acquire) as u32
    }
    fn send(&self, command: ServiceCommand) -> Result<(), AudioError> {
        self.sender.try_send(command).map_err(|error| {
            AudioError::Device(format!("audio command queue unavailable: {error}"))
        })
    }
}

impl Drop for AudioService {
    fn drop(&mut self) {
        let _ = self.sender.try_send(ServiceCommand::Shutdown);
    }
}

fn run_audio_service(
    receiver: Receiver<ServiceCommand>,
    has_audio: Arc<AtomicBool>,
    failed: Arc<AtomicBool>,
    clock: Arc<AtomicI64>,
    playing: Arc<AtomicBool>,
    sample_rate: Arc<AtomicI64>,
) {
    let host = cpal::default_host();
    let Some(device) = host.default_output_device() else {
        failed.store(true, Ordering::Release);
        return;
    };
    let Ok(default_config) = device.default_output_config() else {
        failed.store(true, Ordering::Release);
        return;
    };
    let config = default_config.config();
    sample_rate.store(config.sample_rate.0 as i64, Ordering::Release);
    let panic_muted = Arc::new(AtomicBool::new(false));
    let test_remaining = Arc::new(AtomicI64::new(0));
    let stream = match default_config.sample_format() {
        cpal::SampleFormat::F32 => build_demo_stream::<f32>(
            &device,
            &config,
            &playing,
            &panic_muted,
            &clock,
            &test_remaining,
            &has_audio,
            &failed,
        ),
        cpal::SampleFormat::I16 => build_demo_stream::<i16>(
            &device,
            &config,
            &playing,
            &panic_muted,
            &clock,
            &test_remaining,
            &has_audio,
            &failed,
        ),
        cpal::SampleFormat::U16 => build_demo_stream::<u16>(
            &device,
            &config,
            &playing,
            &panic_muted,
            &clock,
            &test_remaining,
            &has_audio,
            &failed,
        ),
        _ => {
            failed.store(true, Ordering::Release);
            return;
        }
    };
    let Ok(stream) = stream else {
        failed.store(true, Ordering::Release);
        return;
    };
    if stream.play().is_err() {
        failed.store(true, Ordering::Release);
        return;
    }
    loop {
        match receiver.try_recv() {
            Ok(ServiceCommand::Play) => {
                clock.store(0, Ordering::Release);
                panic_muted.store(false, Ordering::Release);
                playing.store(true, Ordering::Release);
            }
            Ok(ServiceCommand::Stop) => playing.store(false, Ordering::Release),
            Ok(ServiceCommand::PanicMute) => {
                playing.store(false, Ordering::Release);
                panic_muted.store(true, Ordering::Release);
            }
            Ok(ServiceCommand::TestTone) => {
                test_remaining.store(config.sample_rate.0 as i64, Ordering::Release)
            }
            Ok(ServiceCommand::Shutdown) => break,
            Err(TryRecvError::Empty) => thread::sleep(Duration::from_millis(4)),
            Err(TryRecvError::Disconnected) => break,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn build_demo_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    playing: &Arc<AtomicBool>,
    panic_muted: &Arc<AtomicBool>,
    sample_clock: &Arc<AtomicI64>,
    test_remaining: &Arc<AtomicI64>,
    callback_has_audio: &Arc<AtomicBool>,
    device_failed: &Arc<AtomicBool>,
) -> Result<cpal::Stream, cpal::BuildStreamError>
where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    let playing = Arc::clone(playing);
    let panic_muted = Arc::clone(panic_muted);
    let clock = Arc::clone(sample_clock);
    let test_remaining = Arc::clone(test_remaining);
    let has_audio = Arc::clone(callback_has_audio);
    let failed = Arc::clone(device_failed);
    let channels = config.channels as usize;
    let rate = config.sample_rate.0 as i64;
    device.build_output_stream(
        config,
        move |data: &mut [T], _| {
            let active = playing.load(Ordering::Acquire) && !panic_muted.load(Ordering::Acquire);
            let mut position = clock.load(Ordering::Relaxed);
            let mut test_left = test_remaining.load(Ordering::Relaxed);
            let mut non_zero = false;
            for frame in data.chunks_mut(channels) {
                let time = position as f32 / rate as f32;
                let backing = if active {
                    (std::f32::consts::TAU * 110.0 * time).sin() * 0.045
                } else {
                    0.0
                };
                let click_phase = position.rem_euclid(rate / 2);
                let click = if active && click_phase < 720 {
                    (std::f32::consts::TAU * 1400.0 * click_phase as f32 / rate as f32).sin()
                        * (1.0 - click_phase as f32 / 720.0)
                        * 0.35
                } else {
                    0.0
                };
                let cue_phase = position.rem_euclid(rate * 8) - rate * 3;
                let cue = if active && (0..rate / 3).contains(&cue_phase) {
                    (std::f32::consts::TAU * 660.0 * cue_phase as f32 / rate as f32).sin() * 0.18
                } else {
                    0.0
                };
                let test = if test_left > 0 {
                    test_left -= 1;
                    (std::f32::consts::TAU * 440.0 * time).sin() * 0.2
                } else {
                    0.0
                };
                for (channel, output) in frame.iter_mut().enumerate() {
                    let value = if channels >= 4 {
                        match channel {
                            0 | 1 => backing + test,
                            2 => click + test,
                            3 => cue + test,
                            _ => test,
                        }
                    } else {
                        backing + click + cue + test
                    };
                    non_zero |= value.abs() > 0.0001;
                    *output = T::from_sample(value.clamp(-1.0, 1.0));
                }
                if active {
                    position += 1;
                }
            }
            clock.store(position, Ordering::Release);
            test_remaining.store(test_left, Ordering::Release);
            if non_zero {
                has_audio.store(true, Ordering::Release);
            }
        },
        move |_| failed.store(true, Ordering::Release),
        None,
    )
}

pub struct CpalBackend {
    stream: Option<cpal::Stream>,
    control: RealtimeControl,
}

impl CpalBackend {
    pub fn new(control: RealtimeControl) -> Self {
        Self {
            stream: None,
            control,
        }
    }
}

impl AudioBackend for CpalBackend {
    fn output_devices(&self) -> Result<Vec<AudioDeviceInfo>, AudioError> {
        let host = cpal::default_host();
        let devices = host
            .output_devices()
            .map_err(|e| AudioError::Device(e.to_string()))?;
        let mut output = Vec::new();
        for (index, device) in devices.enumerate() {
            let name = device.name().unwrap_or_else(|_| format!("Output {index}"));
            let mut channels = 0_u16;
            let mut rates = Vec::new();
            if let Ok(configs) = device.supported_output_configs() {
                for config in configs {
                    channels = channels.max(config.channels());
                    rates.push(config.min_sample_rate().0);
                    rates.push(config.max_sample_rate().0);
                }
            }
            rates.sort_unstable();
            rates.dedup();
            output.push(AudioDeviceInfo {
                id: AudioDeviceId(name.clone()),
                name,
                output_channels: channels,
                sample_rates: rates,
            });
        }
        Ok(output)
    }

    fn open(
        &mut self,
        device_id: &AudioDeviceId,
        requested: StreamConfig,
    ) -> Result<(), AudioError> {
        let host = cpal::default_host();
        let device = host
            .output_devices()
            .map_err(|e| AudioError::Device(e.to_string()))?
            .find(|d| d.name().ok().as_deref() == Some(device_id.0.as_str()))
            .ok_or_else(|| {
                AudioError::Device(format!("output device {} is unavailable", device_id.0))
            })?;
        let supported = device
            .supported_output_configs()
            .map_err(|e| AudioError::Device(e.to_string()))?
            .find(|c| {
                c.channels() == requested.channels
                    && c.sample_format() == cpal::SampleFormat::F32
                    && c.min_sample_rate().0 <= requested.sample_rate
                    && c.max_sample_rate().0 >= requested.sample_rate
            })
            .ok_or_else(|| {
                AudioError::Device(
                    "requested f32 channel/sample-rate configuration is unsupported".into(),
                )
            })?;
        let config = supported
            .with_sample_rate(cpal::SampleRate(requested.sample_rate))
            .config();
        let commands = Arc::clone(&self.control.commands);
        let clock = Arc::clone(&self.control.sample_clock);
        let failed = Arc::clone(&self.control.device_failed);
        let channels = requested.channels as usize;
        let stream = device
            .build_output_stream(
                &config,
                move |data: &mut [f32], _| {
                    while let Some(command) = commands.pop() {
                        match command {
                            TransportCommand::Play { start_sample } => {
                                clock.store(start_sample, Ordering::Release)
                            }
                            TransportCommand::Pause
                            | TransportCommand::Stop { .. }
                            | TransportCommand::PanicMute => {}
                        }
                    }
                    data.fill(0.0);
                    clock.fetch_add((data.len() / channels) as i64, Ordering::Release);
                },
                move |_| {
                    failed.store(true, Ordering::Release);
                },
                None,
            )
            .map_err(|e| AudioError::Device(e.to_string()))?;
        stream
            .play()
            .map_err(|e| AudioError::Device(e.to_string()))?;
        self.stream = Some(stream);
        Ok(())
    }

    fn stop(&mut self) {
        self.stream = None;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamConfig {
    pub sample_rate: u32,
    pub channels: u16,
    pub buffer_frames: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportCommand {
    Play { start_sample: i64 },
    Pause,
    Stop { fade_samples: u32 },
    PanicMute,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransportSnapshot {
    pub sample_clock: i64,
    pub playing: bool,
    pub panic_muted: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ScheduledKind {
    Backing,
    Click { accent: bool },
    Cue,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScheduledEvent {
    pub sample: i64,
    pub bus_id: String,
    pub kind: ScheduledKind,
    pub gain: f32,
}

#[derive(Debug, Error)]
pub enum AudioError {
    #[error("timing error: {0}")]
    Timing(#[from] TimingError),
    #[error("WAV error: {0}")]
    Wav(#[from] hound::Error),
    #[error("unsupported WAV format")]
    UnsupportedWav,
    #[error("invalid output routing: {0}")]
    InvalidRouting(String),
    #[error("audio device error: {0}")]
    Device(String),
}

pub trait SpeechProvider {
    fn render(
        &self,
        text: &str,
        output: &Path,
        pronunciation: &HashMap<String, String>,
    ) -> Result<(), AudioError>;
}

/// Edit-time platform fallback. Never call this from an audio callback.
pub struct PlatformSpeech;
impl SpeechProvider for PlatformSpeech {
    fn render(
        &self,
        text: &str,
        output: &Path,
        pronunciation: &HashMap<String, String>,
    ) -> Result<(), AudioError> {
        let spoken = pronunciation
            .iter()
            .fold(text.to_owned(), |value, (from, to)| value.replace(from, to));
        #[cfg(target_os = "macos")]
        let status = Command::new("say")
            .args([
                "-o",
                output
                    .to_str()
                    .ok_or_else(|| AudioError::Device("invalid output path".into()))?,
                &spoken,
            ])
            .status();
        #[cfg(target_os = "windows")]
        let status = Command::new("powershell").args(["-NoProfile", "-Command", &format!("Add-Type -AssemblyName System.Speech; $s=New-Object System.Speech.Synthesis.SpeechSynthesizer; $s.SetOutputToWaveFile('{}'); $s.Speak('{}')", output.display(), spoken.replace('\'', "''"))]).status();
        #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
        let status = Command::new("espeak")
            .args([
                "-w",
                output
                    .to_str()
                    .ok_or_else(|| AudioError::Device("invalid output path".into()))?,
                &spoken,
            ])
            .status();
        match status {
            Ok(value) if value.success() => Ok(()),
            Ok(value) => Err(AudioError::Device(format!(
                "speech provider exited with {value}"
            ))),
            Err(error) => Err(AudioError::Device(error.to_string())),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DecodedAudio {
    pub sample_rate: u32,
    pub channels: u16,
    /// Interleaved, normalized floating-point samples.
    pub samples: Vec<f32>,
}

pub fn decode_wav(path: impl AsRef<Path>) -> Result<DecodedAudio, AudioError> {
    let mut reader = hound::WavReader::open(path)?;
    let spec = reader.spec();
    let samples = match (spec.sample_format, spec.bits_per_sample) {
        (hound::SampleFormat::Float, 32) => {
            reader.samples::<f32>().collect::<Result<Vec<_>, _>>()?
        }
        (hound::SampleFormat::Int, bits @ 1..=32) => {
            let scale = (1_i64 << (bits - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|v| v.map(|s| s as f32 / scale))
                .collect::<Result<Vec<_>, _>>()?
        }
        _ => return Err(AudioError::UnsupportedWav),
    };
    Ok(DecodedAudio {
        sample_rate: spec.sample_rate,
        channels: spec.channels,
        samples,
    })
}

pub fn validate_routing(buses: &[OutputBus], available_channels: u16) -> Result<(), AudioError> {
    for bus in buses {
        if bus.hardware_channels.len() != bus.channel_count as usize {
            return Err(AudioError::InvalidRouting(format!(
                "bus {} width mismatch",
                bus.name
            )));
        }
        for channel in &bus.hardware_channels {
            if *channel == 0 || *channel > available_channels {
                return Err(AudioError::InvalidRouting(format!(
                    "bus {} requests unavailable channel {}",
                    bus.name, channel
                )));
            }
        }
    }
    Ok(())
}

/// Builds the immutable event list consumed by a future real-time callback.
pub fn fixed_click_schedule(
    timeline: &Timeline,
    first: MusicalPosition,
    bars: u32,
    numerator: u16,
    sample_rate: u32,
    click_bus: &str,
) -> Result<Vec<ScheduledEvent>, AudioError> {
    let mut events = Vec::with_capacity(bars as usize * numerator as usize);
    for bar_offset in 0..bars {
        for beat in 1..=numerator {
            let position = MusicalPosition::new(first.bar + bar_offset as i32, beat, 0);
            events.push(ScheduledEvent {
                sample: timeline.position_to_samples(position, sample_rate)?,
                bus_id: click_bus.to_owned(),
                kind: ScheduledKind::Click { accent: beat == 1 },
                gain: if beat == 1 { 1.0 } else { 0.65 },
            });
        }
    }
    Ok(events)
}

#[derive(Debug, Clone)]
pub struct OfflineRenderSpec {
    pub sample_rate: u32,
    pub seconds: f64,
    pub channels: u16,
    pub backing_hz: f32,
    pub events: Vec<ScheduledEvent>,
    pub buses: Vec<OutputBus>,
}

/// Deterministic test render: backing sine, short click impulses and cue tones.
pub fn render_offline(path: impl AsRef<Path>, spec: &OfflineRenderSpec) -> Result<(), AudioError> {
    validate_routing(&spec.buses, spec.channels)?;
    let frames = (spec.seconds * spec.sample_rate as f64).ceil() as usize;
    let mut output = vec![0.0_f32; frames * spec.channels as usize];
    if let Some(backing) = spec
        .buses
        .iter()
        .find(|b| b.name.eq_ignore_ascii_case("backing"))
    {
        for frame in 0..frames {
            let value = (std::f32::consts::TAU * spec.backing_hz * frame as f32
                / spec.sample_rate as f32)
                .sin()
                * 0.1;
            for channel in &backing.hardware_channels {
                output[frame * spec.channels as usize + (*channel as usize - 1)] += value;
            }
        }
    }
    for event in &spec.events {
        let Some(bus) = spec.buses.iter().find(|b| b.id == event.bus_id) else {
            return Err(AudioError::InvalidRouting(format!(
                "missing bus {}",
                event.bus_id
            )));
        };
        if event.sample < 0 {
            continue;
        }
        let start = event.sample as usize;
        let length = match event.kind {
            ScheduledKind::Click { .. } => 96,
            ScheduledKind::Cue => spec.sample_rate as usize / 20,
            ScheduledKind::Backing => 0,
        };
        for offset in 0..length.min(frames.saturating_sub(start)) {
            let envelope = 1.0 - offset as f32 / length as f32;
            let value = match event.kind {
                ScheduledKind::Click { accent } => {
                    if offset == 0 {
                        event.gain * if accent { 0.9 } else { 0.6 }
                    } else {
                        0.0
                    }
                }
                ScheduledKind::Cue => {
                    (std::f32::consts::TAU * 880.0 * offset as f32 / spec.sample_rate as f32).sin()
                        * 0.25
                        * envelope
                }
                ScheduledKind::Backing => 0.0,
            };
            for channel in &bus.hardware_channels {
                output[(start + offset) * spec.channels as usize + (*channel as usize - 1)] +=
                    value;
            }
        }
    }
    let wav_spec = hound::WavSpec {
        channels: spec.channels,
        sample_rate: spec.sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path, wav_spec)?;
    for sample in output {
        writer.write_sample(sample.clamp(-1.0, 1.0))?;
    }
    writer.finalize()?;
    Ok(())
}

pub fn role_kind(role: AudioAssetRole) -> ScheduledKind {
    match role {
        AudioAssetRole::Backing | AudioAssetRole::Stem | AudioAssetRole::Reference => {
            ScheduledKind::Backing
        }
        AudioAssetRole::Cue => ScheduledKind::Cue,
        AudioAssetRole::Click => ScheduledKind::Click { accent: false },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stagecue_project_model::{MeterChange, TempoChange, TempoTransition};

    fn bus(id: &str, name: &str, channels: &[u16]) -> OutputBus {
        OutputBus {
            id: id.into(),
            name: name.into(),
            channel_count: channels.len() as u8,
            gain_db: 0.0,
            muted: false,
            soloed: false,
            hardware_channels: channels.to_vec(),
        }
    }

    #[test]
    fn offline_render_places_backing_click_and_cue_on_expected_channels() {
        let timeline = Timeline::new(
            vec![MeterChange {
                position: MusicalPosition::new(1, 1, 0),
                numerator: 4,
                denominator: 4,
            }],
            vec![TempoChange {
                position: MusicalPosition::new(1, 1, 0),
                bpm: 120.0,
                transition: TempoTransition::Instant,
            }],
        )
        .unwrap();
        let mut events = fixed_click_schedule(
            &timeline,
            MusicalPosition::new(1, 1, 0),
            1,
            4,
            48_000,
            "click",
        )
        .unwrap();
        events.push(ScheduledEvent {
            sample: 24_000,
            bus_id: "cue".into(),
            kind: ScheduledKind::Cue,
            gain: 1.0,
        });
        let buses = vec![
            bus("backing", "Backing", &[1, 2]),
            bus("click", "Click", &[3]),
            bus("cue", "Cue", &[4]),
        ];
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("render.wav");
        render_offline(
            &path,
            &OfflineRenderSpec {
                sample_rate: 48_000,
                seconds: 2.0,
                channels: 4,
                backing_hz: 220.0,
                events,
                buses,
            },
        )
        .unwrap();
        let decoded = decode_wav(&path).unwrap();
        let at = |frame: usize, channel: usize| decoded.samples[frame * 4 + channel];
        assert!(at(100, 0).abs() > 0.001 && at(100, 1).abs() > 0.001);
        assert!(at(0, 2).abs() > 0.5);
        assert!(at(24_000 + 10, 3).abs() > 0.01);
        assert_eq!(at(100, 3), 0.0);
    }

    #[test]
    fn rejects_unavailable_hardware_channel() {
        assert!(validate_routing(&[bus("cue", "Cue", &[5])], 4).is_err());
    }
}
