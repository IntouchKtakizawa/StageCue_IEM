# StageCue architecture

## Plan

1. Foundation: schemas, fixtures, validation, migration, timing math, tests — implemented.
2. Audio proof of concept: CPAL device adapter, WAV decoding, precomputed schedules, offline multichannel render — implemented; live mixing remains integration work.
3. Editor: waveform timeline, section editing, transport shell, undo/redo — implemented.
4. Templates and persistence: wizard, atomic saves, backups, recovery snapshots — implemented.
5. Performance mode: preflight model, setlists, routing confirmation, safety controls — implemented.

## Repository tree

```text
stagecue/
  apps/desktop/              Tauri + React application (later stage)
  crates/
    audio-analysis/          waveform/transient/BPM analysis boundary
    audio-engine/            real-time scheduling and backend boundary
    project-model/           schema, validation, migration
    timing-engine/           authoritative musical-time conversion
  packages/
    shared-types/            generated/transport-only TypeScript types
    ui/                      reusable React UI (later stage)
  templates/                 versioned user-editable templates
  fixtures/                  deterministic test projects
  docs/                      architecture and schema notes
  tests/                     cross-crate/integration tests (later stages)
```

## Decisions and trade-offs

- **Rust is authoritative.** TypeScript may display returned positions but must not independently calculate sample positions.
- **Audio backend ports.** `audio-engine` exposes backend-neutral commands and snapshots. CPAL can be the first adapter; ASIO/JUCE can replace it without changing project or timing crates.
- **Real-time isolation.** The future callback consumes immutable, predecoded buffers and a precomputed sample schedule through bounded lock-free queues. It must not allocate, lock, log, parse, or call the UI.
- **Position convention.** Serialized/UI bars and beats are one-based; ticks are zero-based. Bar 1 beat 1 tick 0 is absolute tick 0. Bar 0 and negative bars represent count-in time. Internal timeline arithmetic uses signed absolute ticks.
- **Tempo ramps.** A `linear` tempo marker ramps BPM linearly in musical ticks until the next marker. Time is calculated using the exact logarithmic integral, avoiding block-size-dependent drift.
- **Portable data.** IDs are strings and paths may be relative when packaged. Versioned migrations return a new value; persistence must atomically save it beside the original before replacement.
- **Scope discipline.** SQLite indexes the local library later; the portable song remains JSON. Audio and TTS are never embedded as timing logic.
