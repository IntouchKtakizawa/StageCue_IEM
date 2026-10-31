import { useEffect, useRef, useState } from "react";
import type { SongProject, SongSection } from "@stagecue/shared-types";
import { invoke } from "@tauri-apps/api/core";
import { createFromTemplate, useStageCue } from "./store";

const nativeAudio = async (command: string) => {
  if ("__TAURI_INTERNALS__" in window) await invoke(command);
};

type PositionStatus = {
  bar: number;
  beat: number;
  elapsedSeconds: number;
  currentSection: string;
  nextSection?: string;
  barsRemaining: number;
  playing: boolean;
};
const initialPosition: PositionStatus = {
  bar: 1,
  beat: 1,
  elapsedSeconds: 0,
  currentSection: "Intro",
  barsRemaining: 4,
  playing: false,
};
function useTransportPosition() {
  const project = useStageCue((state) => state.project);
  const [position, setPosition] = useState(initialPosition);
  useEffect(() => {
    if (!("__TAURI_INTERNALS__" in window)) return;
    const update = () =>
      void invoke<PositionStatus>("audio_position", {
        bpm: project.tempoMap[0].bpm,
        sections: project.sections,
      })
        .then(setPosition)
        .catch(() => undefined);
    update();
    const timer = window.setInterval(update, 80);
    return () => window.clearInterval(timer);
  }, [project.tempoMap, project.sections]);
  return position;
}
const clockText = (seconds: number) =>
  `${Math.floor(seconds / 60)
    .toString()
    .padStart(2, "0")}:${Math.floor(seconds % 60)
    .toString()
    .padStart(2, "0")}`;

const sectionColors: Record<string, string> = {
  intro: "#64748b",
  verse: "#2563eb",
  chorus: "#e11d48",
  prechorus: "#d97706",
  bridge: "#7c3aed",
  breakdown: "#0891b2",
  solo: "#16a34a",
  outro: "#475569",
};
const analyzeLyrics = (lyrics: string): SongSection[] => {
  const blocks = lyrics
    .trim()
    .split(/\n\s*\n/)
    .map((block) => block.trim())
    .filter(Boolean);
  if (!blocks.length) return [];
  const counts = new Map<string, number>();
  const normalized = blocks.map((block) =>
    block
      .replace(/^\s*\[.*?\]\s*/i, "")
      .toLowerCase()
      .replace(/\W/g, ""),
  );
  normalized.forEach((key) => counts.set(key, (counts.get(key) ?? 0) + 1));
  const occurrence = new Map<string, number>();
  let bar = 1;
  let verse = 0;
  return blocks.map((block, index) => {
    const heading =
      block.match(
        /^\s*\[([^\]]+)\]|^\s*(intro|verse(?:\s+\d+)?|pre-?chorus|chorus(?:\s+\d+)?|bridge|breakdown|solo|outro)\s*:?\s*$/im,
      )?.[1] ??
      block.match(
        /^\s*(intro|verse(?:\s+\d+)?|pre-?chorus|chorus(?:\s+\d+)?|bridge|breakdown|solo|outro)\s*:?\s*$/im,
      )?.[1];
    let base = heading?.trim();
    if (!base) {
      if ((counts.get(normalized[index]) ?? 0) > 1) base = "Chorus";
      else if (index === 0 && block.split("\n").length <= 2) base = "Intro";
      else base = `Verse ${++verse}`;
    }
    const key = base.toLowerCase().replace(/[^a-z]/g, "");
    const number = (occurrence.get(base) ?? 0) + 1;
    occurrence.set(base, number);
    const name = number > 1 && !/\d/.test(base) ? `${base} ${number}` : base;
    const lines = block
      .split("\n")
      .filter((line) => !/^\s*\[.*\]\s*$/.test(line)).length;
    const lengthBars = Math.max(4, Math.ceil(lines / 4) * 4);
    const section = {
      id: crypto.randomUUID(),
      name,
      start: { bar, beat: 1, tick: 0 },
      lengthBars,
      color: sectionColors[key] ?? "#2563eb",
      notes: block,
    };
    bar += lengthBars;
    return section;
  });
};

export function App() {
  const { project, page, setPage, replaceProject } = useStageCue();
  const [showWizard, setShowWizard] = useState(false);
  const input = useRef<HTMLInputElement>(null);
  const save = () => {
    const blob = new Blob([JSON.stringify(project, null, 2)], {
      type: "application/json",
    });
    const a = document.createElement("a");
    a.href = URL.createObjectURL(blob);
    a.download = `${project.title.replace(/\W+/g, "-").toLowerCase()}.stagecue.json`;
    a.click();
    URL.revokeObjectURL(a.href);
  };
  const open = async (file?: File) => {
    if (!file) return;
    replaceProject(JSON.parse(await file.text()) as SongProject);
  };
  return (
    <main
      className={page === "performance" ? "performance-shell" : "app-shell"}
    >
      {page !== "performance" && (
        <header className="brandbar">
          <div>
            <strong>StageCue</strong>
            <span>offline show control</span>
          </div>
          <nav>
            <button onClick={() => setShowWizard(true)}>
              New from template
            </button>
            <button onClick={() => input.current?.click()}>Open</button>
            <button onClick={save}>Save copy</button>
            <input
              ref={input}
              hidden
              type="file"
              accept=".json,.stagecue"
              onChange={(e) => open(e.target.files?.[0])}
            />
          </nav>
        </header>
      )}
      {page !== "performance" && (
        <nav className="tabs" aria-label="Workspace">
          <button
            className={page === "song" ? "active" : ""}
            onClick={() => setPage("song")}
          >
            Song setup
          </button>
          <button
            className={page === "editor" ? "active" : ""}
            onClick={() => setPage("editor")}
          >
            Song mapper
          </button>
          <button
            className={page === "routing" ? "active" : ""}
            onClick={() => setPage("routing")}
          >
            Mixer & routing
          </button>
          <button
            className={page === "setlist" ? "active" : ""}
            onClick={() => setPage("setlist")}
          >
            Setlist
          </button>
          <button className="perform" onClick={() => setPage("performance")}>
            Performance mode
          </button>
        </nav>
      )}
      {page === "song" && <SongSetup />}
      {page === "editor" && <Editor />}
      {page === "routing" && <Routing />}
      {page === "setlist" && <Setlist />}
      {page === "performance" && <Performance />}
      {showWizard && <TemplateWizard close={() => setShowWizard(false)} />}
    </main>
  );
}

function SongSetup() {
  const { project, updateSong, setTempo, replaceSections, setPage } =
    useStageCue();
  const [draftLyrics, setDraftLyrics] = useState(project.lyrics ?? "");
  const [suggestions, setSuggestions] = useState<SongSection[]>([]);
  const spotifyOk =
    !project.spotifyUrl ||
    /^https:\/\/open\.spotify\.com\/track\/[A-Za-z0-9]+/.test(
      project.spotifyUrl,
    );
  const analyze = () => {
    updateSong({ lyrics: draftLyrics });
    setSuggestions(analyzeLyrics(draftLyrics));
  };
  return (
    <section className="song-setup">
      <div className="page-title">
        <div>
          <span className="eyebrow">PROJECT FOUNDATION</span>
          <h1>Build the song map</h1>
          <p>
            Add the real song details, then turn your pasted lyrics into an
            editable live arrangement.
          </p>
        </div>
        <span className="status-chip">LOCAL · PRIVATE</span>
      </div>
      <div className="setup-grid">
        <article className="panel song-details">
          <header>
            <div>
              <span className="step">01</span>
              <h2>Song details</h2>
            </div>
            <small>Saved inside the .stagecue file</small>
          </header>
          <div className="form-grid">
            <label>
              Song title
              <input
                value={project.title}
                onChange={(e) => updateSong({ title: e.target.value })}
              />
            </label>
            <label>
              Artist
              <input
                value={project.artist ?? ""}
                onChange={(e) => updateSong({ artist: e.target.value })}
                placeholder="Artist / band"
              />
            </label>
            <label>
              Tempo (BPM)
              <input
                type="number"
                min="20"
                max="300"
                value={project.tempoMap[0].bpm}
                onChange={(e) => setTempo(+e.target.value)}
              />
            </label>
            <label>
              Time signature
              <input
                value="4 / 4"
                disabled
                title="Variable meter editing is coming next"
              />
            </label>
          </div>
          <label>
            Band notes
            <textarea
              value={project.notes}
              onChange={(e) => updateSong({ notes: e.target.value })}
              placeholder="Key, tuning, count-in, stage notes…"
            />
          </label>
        </article>
        <article className="panel spotify-card">
          <header>
            <div>
              <span className="step">02</span>
              <h2>Spotify reference</h2>
            </div>
            <span className="spotify-dot">●</span>
          </header>
          <p>
            Link the official track for reference. StageCue opens it in Spotify;
            it does not copy or stream Spotify audio.
          </p>
          <label>
            Spotify track URL
            <input
              className={!spotifyOk ? "invalid" : ""}
              value={project.spotifyUrl ?? ""}
              onChange={(e) => updateSong({ spotifyUrl: e.target.value })}
              placeholder="https://open.spotify.com/track/…"
            />
          </label>
          {!spotifyOk && (
            <small className="error">
              Use a Spotify track link, not a playlist or search page.
            </small>
          )}
          <div className="button-row">
            <a
              className={`button ${!spotifyOk || !project.spotifyUrl ? "disabled" : ""}`}
              href={spotifyOk ? project.spotifyUrl : undefined}
              target="_blank"
              rel="noreferrer"
            >
              Open in Spotify ↗
            </a>
            <span>Reference only</span>
          </div>
        </article>
        <article className="panel lyrics-card">
          <header>
            <div>
              <span className="step">03</span>
              <h2>Lyrics → arrangement</h2>
            </div>
            <span className="ai-chip">AI ASSIST</span>
          </header>
          <p>
            Paste lyrics you have permission to use. Headings like{" "}
            <code>[Chorus]</code> are respected; repeated blocks are detected as
            choruses.
          </p>
          <textarea
            className="lyrics-input"
            value={draftLyrics}
            onChange={(e) => setDraftLyrics(e.target.value)}
            placeholder={"[Verse 1]\nFirst line…\n\n[Chorus]\nRepeated hook…"}
          />
          <div className="button-row">
            <button
              className="primary"
              onClick={analyze}
              disabled={!draftLyrics.trim()}
            >
              Analyze & name sections
            </button>
            <small>
              {draftLyrics.trim()
                ? `${draftLyrics.trim().split(/\s+/).length} words`
                : "Nothing leaves this Mac"}
            </small>
          </div>
        </article>
        <article className="panel analysis-card">
          <header>
            <div>
              <span className="step">04</span>
              <h2>Review the structure</h2>
            </div>
            <small>
              {suggestions.length
                ? `${suggestions.length} sections found`
                : "Waiting for lyrics"}
            </small>
          </header>
          {suggestions.length ? (
            <>
              <div className="suggestions">
                {suggestions.map((section) => (
                  <div key={section.id}>
                    <i style={{ background: section.color }} />
                    <input
                      value={section.name}
                      onChange={(e) =>
                        setSuggestions(
                          suggestions.map((s) =>
                            s.id === section.id
                              ? { ...s, name: e.target.value }
                              : s,
                          ),
                        )
                      }
                    />
                    <span>{section.lengthBars} bars</span>
                  </div>
                ))}
              </div>
              <div className="button-row">
                <button
                  className="primary"
                  onClick={() => {
                    replaceSections(suggestions);
                    setPage("editor");
                  }}
                >
                  Apply to timeline
                </button>
                <button onClick={() => setSuggestions([])}>Clear</button>
              </div>
            </>
          ) : (
            <div className="empty-state">
              <b>Your proposed set map appears here.</b>
              <span>You can rename every section before anything changes.</span>
            </div>
          )}
        </article>
      </div>
    </section>
  );
}

function Transport() {
  const { playing, togglePlay, stop, panic, project, undo, redo } =
    useStageCue();
  const position = useTransportPosition();
  return (
    <section className="transport">
      <button
        className="play"
        onClick={() => {
          void nativeAudio(playing ? "audio_stop" : "audio_play");
          togglePlay();
        }}
      >
        {playing ? "Ⅱ Pause" : "▶ Play"}
      </button>
      <button
        onClick={() => {
          void nativeAudio("audio_stop");
          stop();
        }}
      >
        ■ Stop
      </button>
      <button onClick={undo}>↶ Undo</button>
      <button onClick={redo}>↷ Redo</button>
      <div>
        <small>POSITION</small>
        <b>
          {position.bar} · {position.beat}
        </b>
      </div>
      <div>
        <small>TEMPO</small>
        <b>{project.tempoMap[0].bpm} BPM</b>
      </div>
      <div>
        <small>METER</small>
        <b>4/4</b>
      </div>
      <div>
        <small>OUTPUT</small>
        <b>Not selected</b>
      </div>
      <button
        className="panic"
        onClick={() => {
          void nativeAudio("audio_panic");
          panic();
        }}
      >
        MUTE
      </button>
    </section>
  );
}

function Editor() {
  const {
    project,
    selectedSectionId,
    selectSection,
    renameSection,
    resizeSection,
    updateSection,
    addSection,
    setBackingFile,
    toggleClick,
    addVoiceCue,
  } = useStageCue();
  const [track, setTrack] = useState<"backing" | "click" | "cues" | "sections">(
    "sections",
  );
  const [cueText, setCueText] = useState("Chorus");
  const [cueBar, setCueBar] = useState(1);
  const backingInput = useRef<HTMLInputElement>(null);
  const cueInput = useRef<HTMLInputElement>(null);
  const selected = project.sections.find((s) => s.id === selectedSectionId);
  const total = Math.max(
    1,
    project.sections.reduce(
      (n, s) => Math.max(n, s.start.bar + s.lengthBars),
      1,
    ),
  );
  return (
    <>
      <Transport />
      <section className="workspace">
        <aside className="tracklist">
          <h3>TRACKS</h3>
          <button
            className={track === "backing" ? "active" : ""}
            onClick={() => setTrack("backing")}
          >
            ▰ Backing
          </button>
          <button
            className={track === "click" ? "active" : ""}
            onClick={() => setTrack("click")}
          >
            ⌁ Click
          </button>
          <button
            className={track === "cues" ? "active" : ""}
            onClick={() => setTrack("cues")}
          >
            ◉ Voice cues
          </button>
          <button
            className={track === "sections" ? "active" : ""}
            onClick={() => setTrack("sections")}
          >
            ⌁ Sections
          </button>
          <input
            ref={backingInput}
            hidden
            type="file"
            accept=".wav"
            onChange={(event) => {
              const file = event.target.files?.[0];
              if (file) setBackingFile(file.name);
            }}
          />
          <input
            ref={cueInput}
            hidden
            type="file"
            accept=".wav,.aiff,.mp3"
            onChange={(event) => {
              const file = event.target.files?.[0];
              if (file) addVoiceCue(cueText, cueBar, file.name);
            }}
          />
        </aside>
        <div className="timeline">
          <div className="ruler">
            {Array.from({ length: Math.min(total, 64) }, (_, i) => (
              <i key={i}>{i + 1}</i>
            ))}
          </div>
          <div className="waveform" aria-label="Waveform placeholder">
            {Array.from({ length: 80 }, (_, i) => (
              <i
                key={i}
                style={{ height: `${20 + Math.abs(Math.sin(i * 1.7)) * 55}%` }}
              />
            ))}
          </div>
          <div className="section-row">
            {project.sections.map((s) => (
              <button
                key={s.id}
                className={s.id === selectedSectionId ? "selected" : ""}
                style={{
                  width: `${Math.max(90, s.lengthBars * 24)}px`,
                  background: s.color,
                }}
                onClick={() => {
                  setTrack("sections");
                  selectSection(s.id);
                }}
              >
                <b>{s.name}</b>
                <small>
                  Bar {s.start.bar} · {s.lengthBars} bars
                </small>
              </button>
            ))}
          </div>
          <div className="cue-row">
            {project.events
              .filter((event) => event.type === "voice-cue")
              .map((event, index) => (
                <span key={event.id} style={{ left: `${10 + index * 18}%` }}>
                  ◆ {event.text}
                </span>
              ))}
          </div>
          <button className="add" onClick={addSection}>
            ＋ Add section
          </button>
        </div>
        <aside className="inspector">
          <h3>INSPECTOR</h3>
          {track === "backing" ? (
            <>
              <h2>Backing track</h2>
              <p>
                {project.audioAssets.find((asset) => asset.role === "backing")
                  ?.name ?? "No WAV selected"}
              </p>
              <button onClick={() => backingInput.current?.click()}>
                Import WAV backing
              </button>
              <small>
                Copy the selected file into user-content/audio so the portable
                project can find it.
              </small>
            </>
          ) : track === "click" ? (
            <>
              <h2>Click</h2>
              <p>
                {project.click.enabled ? "Enabled" : "Muted"} ·{" "}
                {project.click.subdivision}
              </p>
              <button onClick={toggleClick}>
                {project.click.enabled ? "Disable click" : "Enable click"}
              </button>
              <button onClick={() => void nativeAudio("audio_test_tone")}>
                Preview click output
              </button>
            </>
          ) : track === "cues" ? (
            <>
              <h2>Voice cues</h2>
              <label>
                Spoken text
                <input
                  value={cueText}
                  onChange={(event) => setCueText(event.target.value)}
                />
              </label>
              <label>
                Bar
                <input
                  type="number"
                  min="0"
                  value={cueBar}
                  onChange={(event) => setCueBar(+event.target.value)}
                />
              </label>
              <button onClick={() => addVoiceCue(cueText, cueBar)}>
                Add generated cue
              </button>
              <button onClick={() => cueInput.current?.click()}>
                Import my recorded cue
              </button>
              <p>
                {
                  project.events.filter((event) => event.type === "voice-cue")
                    .length
                }{" "}
                cue(s) in this song
              </p>
            </>
          ) : selected ? (
            <>
              <label>
                Name
                <input
                  value={selected.name}
                  onChange={(e) => renameSection(selected.id, e.target.value)}
                />
              </label>
              <div className="pair">
                <label>
                  Start bar
                  <input type="number" value={selected.start.bar} readOnly />
                </label>
                <label>
                  Length
                  <input
                    type="number"
                    value={selected.lengthBars}
                    onChange={(e) =>
                      resizeSection(selected.id, +e.target.value)
                    }
                  />
                </label>
              </div>
              <label>
                Color
                <input
                  type="color"
                  value={selected.color ?? "#2563eb"}
                  onChange={(e) =>
                    updateSection(selected.id, { color: e.target.value })
                  }
                />
              </label>
              <label>
                Notes
                <textarea
                  value={selected.notes ?? ""}
                  onChange={(e) =>
                    updateSection(selected.id, { notes: e.target.value })
                  }
                  placeholder="Performance notes"
                />
              </label>
            </>
          ) : (
            <p>Select a section block, or add a new section.</p>
          )}
        </aside>
      </section>
    </>
  );
}

function Routing() {
  const { project, routingConfirmed, confirmRouting } = useStageCue();
  return (
    <section className="routing">
      <div className="page-title">
        <div>
          <h1>Mixer & routing</h1>
          <p>Choose exactly where each signal leaves the interface.</p>
        </div>
        <span className={routingConfirmed ? "ok" : "warn"}>
          {routingConfirmed ? "✓ Routing confirmed" : "! Confirmation required"}
        </span>
      </div>
      <div className="bus-grid">
        {project.buses.map((bus) => (
          <article key={bus.id}>
            <header>
              <b>{bus.name}</b>
              <span>{bus.channelCount === 2 ? "STEREO" : "MONO"}</span>
            </header>
            <div className="meter">
              <i />
              <i />
            </div>
            <label>
              Gain{" "}
              <input
                type="range"
                min="-60"
                max="6"
                value={bus.gainDb}
                readOnly
              />
            </label>
            <div className="channels">
              Hardware{" "}
              {bus.hardwareChannels.map((c) => (
                <strong key={c}>OUT {c}</strong>
              ))}
            </div>
            <footer>
              <button>Mute</button>
              <button>Solo</button>
              <button onClick={() => void nativeAudio("audio_test_tone")}>
                Test tone
              </button>
            </footer>
          </article>
        ))}
      </div>
      <div className="routing-map">
        <b>Routing summary</b>
        <span>Backing → outputs 1–2</span>
        <span>Click → output 3</span>
        <span>Cues → output 4</span>
      </div>
      <button className="confirm" onClick={confirmRouting}>
        Confirm routing
      </button>
    </section>
  );
}

function Setlist() {
  const { project } = useStageCue();
  const [songs, setSongs] = useState([project.title, "Second Song", "Encore"]);
  return (
    <section className="setlist">
      <div className="page-title">
        <div>
          <h1>Tonight’s set</h1>
          <p>Manual advance is on. Songs will not start automatically.</p>
        </div>
        <b>{songs.length} songs · 12:40 estimated</b>
      </div>
      {songs.map((song, i) => (
        <article key={song}>
          <span className="drag">⠿</span>
          <strong>{i + 1}</strong>
          <div>
            <b>{song}</b>
            <small>{i === 0 ? "Routing checked" : "Needs preflight"}</small>
          </div>
          <button onClick={() => setSongs(songs.filter((_, n) => n !== i))}>
            Remove
          </button>
        </article>
      ))}
      <button className="add">＋ Add current song</button>
    </section>
  );
}

function Performance() {
  const {
    project,
    playing,
    togglePlay,
    stop,
    panic,
    setPage,
    routingConfirmed,
  } = useStageCue();
  const position = useTransportPosition();
  return (
    <section className="performance">
      <header>
        <strong>STAGECUE · PERFORMANCE</strong>
        <span className={routingConfirmed ? "online" : "offline"}>
          {routingConfirmed
            ? "● DEVICE & ROUTING READY"
            : "● PREFLIGHT REQUIRED"}
        </span>
        <button onClick={() => setPage("editor")}>Exit performance</button>
      </header>
      <div className="now">
        <small>CURRENT SECTION</small>
        <h1>{position.currentSection}</h1>
        <div className="count">{position.barsRemaining}</div>
        <p>BARS REMAINING</p>
      </div>
      <div className="next">
        <small>NEXT</small>
        <h2>{position.nextSection ?? "End"}</h2>
        <p>
          {position.nextSection
            ? `Next cue: “${position.nextSection}, two bars”`
            : "No further cues"}
        </p>
      </div>
      <div className="performance-stats">
        <span>
          BAR{" "}
          <b>
            {position.bar} · {position.beat}
          </b>
        </span>
        <span>
          SET <b>1 / 3</b>
        </span>
        <span>
          ELAPSED <b>{clockText(position.elapsedSeconds)}</b>
        </span>
        <span>
          REMAINING <b>03:42</b>
        </span>
      </div>
      <footer>
        <button
          onClick={() => {
            void nativeAudio(playing ? "audio_stop" : "audio_play");
            togglePlay();
          }}
        >
          {playing ? "Ⅱ PAUSE" : "▶ PLAY"}
        </button>
        <button
          onClick={() => {
            void nativeAudio("audio_stop");
            stop();
          }}
        >
          ■ STOP
        </button>
        <button>↺ RESTART</button>
        <button
          className="panic"
          onClick={() => {
            void nativeAudio("audio_panic");
            panic();
          }}
        >
          EMERGENCY MUTE
        </button>
      </footer>
    </section>
  );
}

function TemplateWizard({ close }: { close: () => void }) {
  const replace = useStageCue((s) => s.replaceProject);
  const [template, setTemplate] = useState("rock");
  const [tempo, setTempo] = useState(120);
  const [intro, setIntro] = useState(4);
  const [bridge, setBridge] = useState(8);
  const create = () => {
    replace(createFromTemplate(template, tempo, intro, bridge));
    close();
  };
  return (
    <div className="modal">
      <form
        onSubmit={(e) => {
          e.preventDefault();
          create();
        }}
      >
        <h2>Build from a template</h2>
        <label>
          Style
          <select
            value={template}
            onChange={(e) => setTemplate(e.target.value)}
          >
            <option value="rock">Standard Rock 4/4</option>
            <option value="fast">Fast Up-Tempo</option>
            <option value="ballad">Ballad with Extended Bridge</option>
          </select>
        </label>
        <div className="pair">
          <label>
            Tempo
            <input
              type="number"
              value={tempo}
              onChange={(e) => setTempo(+e.target.value)}
            />
          </label>
          <label>
            Intro bars
            <input
              type="number"
              value={intro}
              onChange={(e) => setIntro(+e.target.value)}
            />
          </label>
        </div>
        <label>
          Bridge bars
          <input
            type="number"
            value={bridge}
            onChange={(e) => setBridge(+e.target.value)}
          />
        </label>
        <footer>
          <button type="button" onClick={close}>
            Cancel
          </button>
          <button className="confirm">Create song</button>
        </footer>
      </form>
    </div>
  );
}
