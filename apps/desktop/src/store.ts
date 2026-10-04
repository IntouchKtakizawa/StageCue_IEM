import { create } from "zustand";
import type { SongProject, SongSection } from "@stagecue/shared-types";

type Page = "song" | "editor" | "routing" | "setlist" | "performance";
type Snapshot = Pick<State, "project" | "selectedSectionId">;
interface State {
  project: SongProject;
  page: Page;
  selectedSectionId?: string;
  playing: boolean;
  routingConfirmed: boolean;
  history: Snapshot[];
  future: Snapshot[];
  setPage(page: Page): void;
  selectSection(id?: string): void;
  togglePlay(): void;
  stop(): void;
  panic(): void;
  renameSection(id: string, name: string): void;
  updateSong(details: Partial<Pick<SongProject, "title" | "artist" | "spotifyUrl" | "lyrics" | "notes">>): void;
  setTempo(bpm: number): void;
  replaceSections(sections: SongSection[]): void;
  updateSection(id: string, changes: Partial<Pick<SongSection, "name" | "lengthBars" | "color" | "notes">>): void;
  resizeSection(id: string, bars: number): void;
  addSection(): void;
  setBackingFile(fileName: string): void;
  toggleClick(): void;
  addVoiceCue(text: string, bar: number, fileName?: string): void;
  confirmRouting(): void;
  undo(): void;
  redo(): void;
  replaceProject(project: SongProject): void;
}

const starter: SongProject = {
  schemaVersion: 1,
  id: "new-song",
  title: "Untitled Song",
  notes: "",
  audioAssets: [],
  tempoMap: [
    { position: { bar: 1, beat: 1, tick: 0 }, bpm: 120, transition: "instant" },
  ],
  meterMap: [
    { position: { bar: 1, beat: 1, tick: 0 }, numerator: 4, denominator: 4 },
  ],
  freeTimeRegions: [],
  sections: [
    {
      id: "intro",
      name: "Intro",
      start: { bar: 1, beat: 1, tick: 0 },
      lengthBars: 4,
      color: "#62748e",
    },
  ],
  events: [],
  click: {
    enabled: true,
    gainDb: -6,
    subdivision: "quarter",
    patterns: [],
    enabledSectionIds: [],
    muteInFreeTime: true,
  },
  buses: [
    {
      id: "backing-bus",
      name: "Backing",
      channelCount: 2,
      gainDb: 0,
      muted: false,
      soloed: false,
      hardwareChannels: [1, 2],
    },
    {
      id: "click-bus",
      name: "Click",
      channelCount: 1,
      gainDb: 0,
      muted: false,
      soloed: false,
      hardwareChannels: [3],
    },
    {
      id: "cue-bus",
      name: "Cues",
      channelCount: 1,
      gainDb: 0,
      muted: false,
      soloed: false,
      hardwareChannels: [4],
    },
  ],
  routing: [],
  performance: {
    safetyFadeMs: 20,
    autoAdvanceSetlist: false,
    stopConfirmation: true,
    beginnerMode: true,
  },
};
const clone = <T>(value: T): T => structuredClone(value);
const mutation = (state: State, edit: (project: SongProject) => void) => {
  const project = clone(state.project);
  edit(project);
  return {
    project,
    history: [
      ...state.history,
      {
        project: clone(state.project),
        selectedSectionId: state.selectedSectionId,
      },
    ],
    future: [],
  };
};
export const useStageCue = create<State>((set) => ({
  project: starter,
  page: "editor",
  playing: false,
  routingConfirmed: false,
  history: [],
  future: [],
  setPage: (page) => set({ page }),
  selectSection: (selectedSectionId) => set({ selectedSectionId }),
  togglePlay: () => set((s) => ({ playing: !s.playing })),
  stop: () => set({ playing: false }),
  panic: () => set({ playing: false }),
  renameSection: (id, name) =>
    set((s) =>
      mutation(s, (p) => {
        const section = p.sections.find((v) => v.id === id);
        if (section) section.name = name;
      }),
    ),
  updateSong: (details) => set((s) => mutation(s, (p) => Object.assign(p, details))),
  setTempo: (bpm) => set((s) => mutation(s, (p) => { p.tempoMap[0].bpm = Math.min(300, Math.max(20, bpm)); })),
  replaceSections: (sections) => set((s) => mutation(s, (p) => { p.sections = sections; })),
  updateSection: (id, changes) => set((s) => mutation(s, (p) => {
    const section = p.sections.find((v) => v.id === id);
    if (section) Object.assign(section, changes);
  })),
  resizeSection: (id, bars) =>
    set((s) =>
      mutation(s, (p) => {
        const section = p.sections.find((v) => v.id === id);
        if (section) section.lengthBars = Math.max(1, bars);
      }),
    ),
  addSection: () =>
    set((s) =>
      mutation(s, (p) => {
        const last = p.sections.at(-1);
        const start = last ? last.start.bar + last.lengthBars : 1;
        p.sections.push({
          id: crypto.randomUUID(),
          name: "New Section",
          start: { bar: start, beat: 1, tick: 0 },
          lengthBars: 8,
          color: "#8b5cf6",
        });
      }),
    ),
  setBackingFile: (fileName) =>
    set((s) =>
      mutation(s, (p) => {
        p.audioAssets = p.audioAssets.filter(
          (asset) => asset.role !== "backing",
        );
        p.audioAssets.push({
          id: "backing",
          name: fileName,
          path: `user-content/audio/${fileName}`,
          role: "backing",
          channels: 2,
          offsetSamples: 0,
          gainDb: 0,
          muted: false,
        });
        p.routing = [
          ...p.routing.filter((route) => route.sourceId !== "backing"),
          { sourceId: "backing", destinationBusId: "backing-bus" },
        ];
      }),
    ),
  toggleClick: () =>
    set((s) =>
      mutation(s, (p) => {
        p.click.enabled = !p.click.enabled;
      }),
    ),
  addVoiceCue: (text, bar, fileName) =>
    set((s) =>
      mutation(s, (p) => {
        const id = crypto.randomUUID();
        const assetId = `cue-audio-${id}`;
        p.audioAssets.push({
          id: assetId,
          name: fileName ?? text,
          path: `user-content/cues/${fileName ?? `${id}.wav`}`,
          role: "cue",
          channels: 1,
          offsetSamples: 0,
          gainDb: 0,
          muted: false,
        });
        p.events.push({
          type: "voice-cue",
          id: `cue-${id}`,
          timing: { kind: "musical", position: { bar, beat: 1, tick: 0 } },
          text,
          renderedAudioAssetId: assetId,
          destinationBusId: "cue-bus",
          gainDb: 0,
          interruptPolicy: "mix",
          pronunciationOverrides: {},
        });
        p.routing.push({ sourceId: assetId, destinationBusId: "cue-bus" });
      }),
    ),
  confirmRouting: () => set({ routingConfirmed: true }),
  undo: () =>
    set((s) => {
      const previous = s.history.at(-1);
      return previous
        ? {
            ...previous,
            history: s.history.slice(0, -1),
            future: [
              {
                project: clone(s.project),
                selectedSectionId: s.selectedSectionId,
              },
              ...s.future,
            ],
          }
        : {};
    }),
  redo: () =>
    set((s) => {
      const next = s.future[0];
      return next
        ? {
            ...next,
            history: [
              ...s.history,
              {
                project: clone(s.project),
                selectedSectionId: s.selectedSectionId,
              },
            ],
            future: s.future.slice(1),
          }
        : {};
    }),
  replaceProject: (project) =>
    set({
      project,
      history: [],
      future: [],
      selectedSectionId: undefined,
      routingConfirmed: false,
    }),
}));

export function createFromTemplate(
  name: string,
  tempo: number,
  introBars: number,
  bridgeBars: number,
): SongProject {
  const layouts: Record<string, Array<[string, number, string]>> = {
    rock: [
      ["Intro", introBars, "#64748b"],
      ["Verse 1", 8, "#2563eb"],
      ["Chorus 1", 8, "#dc2626"],
      ["Verse 2", 8, "#2563eb"],
      ["Chorus 2", 8, "#dc2626"],
      ["Bridge", bridgeBars, "#7c3aed"],
      ["Final Chorus", 8, "#e11d48"],
      ["Outro", 4, "#475569"],
    ],
    fast: [
      ["Intro", introBars, "#64748b"],
      ["Verse", 8, "#0891b2"],
      ["Chorus", 8, "#f97316"],
      ["Bridge", bridgeBars, "#8b5cf6"],
      ["Final Chorus", 8, "#ef4444"],
    ],
    ballad: [
      ["Intro", introBars, "#64748b"],
      ["Verse 1", 8, "#0ea5e9"],
      ["Chorus", 8, "#f43f5e"],
      ["Bridge", bridgeBars, "#a855f7"],
      ["Final Chorus", 8, "#e11d48"],
      ["Ritardando Outro", 4, "#334155"],
    ],
  };
  let bar = 1;
  const sections: SongSection[] = layouts[name].map(
    ([sectionName, lengthBars, color], index) => {
      const section = {
        id: `section-${index}`,
        name: sectionName,
        start: { bar, beat: 1, tick: 0 },
        lengthBars,
        color,
      };
      bar += lengthBars;
      return section;
    },
  );
  return {
    ...clone(starter),
    id: crypto.randomUUID(),
    title: "New Set Song",
    tempoMap: [
      {
        position: { bar: 1, beat: 1, tick: 0 },
        bpm: tempo,
        transition: "instant",
      },
    ],
    sections,
  };
}
