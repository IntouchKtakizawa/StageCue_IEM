# MVP schema v1

`SongProject` contains identity/notes, assets, tempo and meter maps, optional free-time regions, sections, timeline events, click configuration, buses/routes, and performance settings.

Timeline events use a tagged `type` union. Their placement is a tagged `timing` union:

- `musical`: exact bar/beat/tick
- `seconds`: absolute time from bar 1
- `bars-before-section`
- `beats-before-section`
- `section-boundary`

Relative placements retain a section ID, so moving the section also moves the resolved cue. Rendered voice cues reference an audio asset; synthesis is an edit-time provider concern.

Schema version 1 is defined by the Rust types in `crates/project-model/src/lib.rs`. The matching TypeScript declarations are transport types only, not a second timing implementation.

