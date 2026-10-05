export type ViewId = "home" | "randomizer" | "editor" | "saves" | "settings";

// Miroir des types sérialisés par `kaleido_core::detect`.

export type FileKind = "nds_rom" | "ctr_rom" | "ctr_dump" | "save" | "unknown";
export type Platform = "nds" | "3ds";

export interface GameInfo {
  id: string;
  name: string;
  generation: number;
  platform: Platform;
}

export interface Detail {
  label: string;
  value: string;
}

export interface Detection {
  path: string;
  fileName: string;
  kind: FileKind;
  title: string;
  game: GameInfo | null;
  platform: Platform | null;
  generation: number | null;
  language: string | null;
  isFrench: boolean;
  size: number;
  details: Detail[];
  warnings: string[];
}
