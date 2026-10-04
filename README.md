# StageCue

StageCue is an offline-first Tauri desktop cue, click, and backing-track system for live bands. The repository contains the full MVP architecture and an operational editor/performance workflow.

## Implemented

- Versioned, portable JSON project model with Serde serialization
- Validation with separate errors and warnings
- Schema migration that never mutates the source file
- Authoritative Rust timing engine at 960 PPQ
- Instant tempo changes, analytical linear ramps, meter changes, pickups, and seconds/sample conversion
- JSON fixtures and Rust tests
- CPAL output-device enumeration and a real-time-safe callback/control boundary
- WAV decode, waveform peak extraction, and suggested BPM analysis
- Deterministic four-channel offline render verification
- Atomic project saves, backups, recovery snapshots, and schema migration
- Three editable song templates and a template wizard
- React song mapper, section editing, undo/redo, mixer/routing confirmation, setlist, and performance views
- Tauri commands for native save/load and preflight

The current CPAL callback validates the device/sample-clock and lock-free control design but outputs silence; connecting decoded assets to the precomputed mixer is the remaining production audio integration. Do not use this build for a live show yet.

## Run

```bash
npm install
npm run build
npm test
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
npm run tauri -- dev
```

Current stable Rust and Node 20+ are recommended. No hardware-reliability claim is made without a multichannel interface test.
