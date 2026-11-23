export interface MemeItem {
  id: number;
  sha256: string;
  alias: string;
  tags: string[];
  ext: string;
  mime: string;
  width: number;
  height: number;
  fileSize: number;
  isAnimated: boolean;
  originalPath: string;
  thumbPath: string;
  useCount: number;
  lastUsedAt: number | null;
  createdAt: number;
}

export interface MemePage {
  items: MemeItem[];
  total: number;
  page: number;
  pageSize: number;
  totalPages: number;
}

export interface TagInfo {
  id: number;
  name: string;
  count: number;
}

export interface AppSettings {
  toggleShortcut: string;
  quickCaptureShortcut: string;
  autoSend: boolean;
  qqDockEnabled: boolean;
  pageSize: number;
  theme: "system" | "light" | "dark" | string;
  pasteDelayMs: number;
}

export interface ImportResult {
  imported: number;
  duplicates: number;
  skipped: number;
  errors: string[];
  addedItems: MemeItem[];
}

export interface ScanCandidate {
  path: string;
  sha256: string;
  ext: string;
  mime: string;
  width: number;
  height: number;
  fileSize: number;
  isAnimated: boolean;
  alreadyImported: boolean;
  suggestedAlias: string;
}

export interface QqDirInfo {
  path: string;
  label: string;
  fileCountHint: number;
}

export interface PreviewPayload {
  id: number;
  alias: string;
  tags: string[];
  originalPath: string;
  ext: string;
  width: number;
  height: number;
  fileSize: number;
  isAnimated: boolean;
}

export interface PlatformStatus {
  os: string;
  accessibilityGranted: boolean;
  dataDir: string;
}

export interface PasteResult {
  copied: boolean;
  pasted: boolean;
  needsAccessibility: boolean;
  message: string;
}
