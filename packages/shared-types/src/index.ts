/** Transport-only types. Rust remains authoritative for validation and timing. */
export type MusicalPosition = { bar: number; beat: number; tick: number };
export type TempoChange = {
  position: MusicalPosition;
  bpm: number;
  transition: "instant" | "linear";
};
export type MeterChange = {
  position: MusicalPosition;
  numerator: number;
  denominator: number;
};

export interface SongProject {
  schemaVersion: 1;
  id: string;
  title: string;
  artist?: string;
  spotifyUrl?: string;
  lyrics?: string;
  notes: string;
  audioAssets: AudioAsset[];
  tempoMap: TempoChange[];
  meterMap: MeterChange[];
  freeTimeRegions: Array<{
    id: string;
    startSeconds: number;
    endSeconds: number;
    muteClick: boolean;
  }>;
  sections: SongSection[];
  events: TimelineEvent[];
  click: ClickConfiguration;
  buses: OutputBus[];
  routing: Array<{ sourceId: string; destinationBusId: string }>;
  performance: PerformanceSettings;
}

export interface AudioAsset {
  id: string;
  name: string;
  path: string;
  role: "backing" | "stem" | "reference" | "cue" | "click";
  channels: number;
  sampleRate?: number;
  offsetSamples: number;
  gainDb: number;
  muted: boolean;
}
export interface SongSection {
  id: string;
  name: string;
  start: MusicalPosition;
  lengthBars: number;
  color?: string;
  notes?: string;
}
export type EventTiming =
  | { kind: "musical"; position: MusicalPosition }
  | { kind: "seconds"; seconds: number }
  | { kind: "bars-before-section"; sectionId: string; bars: number }
  | { kind: "beats-before-section"; sectionId: string; beats: number }
  | { kind: "section-boundary"; sectionId: string };
export type TimelineEvent =
  | {
      type: "voice-cue";
      id: string;
      timing: EventTiming;
      text: string;
      renderedAudioAssetId: string;
      destinationBusId: string;
      gainDb: number;
      interruptPolicy: "mix" | "duck-previous" | "skip-if-overlapping";
      pronunciationOverrides: Record<string, string>;
    }
  | {
      type: "count-in";
      id: string;
      timing: EventTiming;
      count: number;
      subdivision: "quarter" | "eighth";
      destinationBusId: string;
    }
  | { type: "marker"; id: string; timing: EventTiming; name: string }
  | {
      type: "midi";
      id: string;
      timing: EventTiming;
      data: number[];
      port?: string;
    }
  | {
      type: "automation";
      id: string;
      timing: EventTiming;
      target: string;
      value: number;
    };
export interface ClickConfiguration {
  enabled: boolean;
  gainDb: number;
  subdivision: "quarter" | "eighth";
  accentedAssetId?: string;
  unaccentedAssetId?: string;
  patterns: Array<{
    numerator: number;
    denominator: number;
    steps: Array<{ beat: number; accent: boolean }>;
  }>;
  enabledSectionIds: string[];
  muteInFreeTime: boolean;
}
export interface OutputBus {
  id: string;
  name: string;
  channelCount: 1 | 2;
  gainDb: number;
  muted: boolean;
  soloed: boolean;
  hardwareChannels: number[];
}
export interface PerformanceSettings {
  safetyFadeMs: number;
  autoAdvanceSetlist: boolean;
  stopConfirmation: boolean;
  beginnerMode: boolean;
}
