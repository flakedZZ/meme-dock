import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import type {
  AppSettings,
  ImportResult,
  MemeItem,
  MemePage,
  PasteResult,
  PlatformStatus,
  QqDirInfo,
  ScanCandidate,
  TagInfo,
} from "./types";

export function fileUrl(path: string): string {
  return convertFileSrc(path);
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
}

export const api = {
  queryMemes: (params: {
    query: string;
    tags: string[];
    page: number;
    pageSize: number;
    sort: string;
  }) => invoke<MemePage>("query_memes", params),

  listTags: () => invoke<TagInfo[]>("list_tags"),

  updateMemeMeta: (id: number, alias: string, tags: string[]) =>
    invoke<MemeItem>("update_meme_meta", { id, alias, tags }),

  deleteMeme: (id: number) => invoke<boolean>("delete_meme", { id }),

  importPaths: (paths: string[], tags: string[] = []) =>
    invoke<ImportResult>("import_paths", { paths, tags }),

  importFromClipboard: (tags: string[] = []) =>
    invoke<ImportResult>("import_from_clipboard", { tags }),

  discoverQqDirs: () => invoke<QqDirInfo[]>("discover_qq_dirs"),

  scanQqDir: (dirPath: string, maxItems = 240) =>
    invoke<ScanCandidate[]>("scan_qq_dir", { dirPath, maxItems }),

  pasteMeme: (id: number, keepOpen: boolean) =>
    invoke<PasteResult>("paste_meme", { id, keepOpen }),

  showPreview: (id: number) => invoke<void>("show_preview", { id }),

  hidePreview: () => invoke<void>("hide_preview"),

  getSettings: () => invoke<AppSettings>("get_settings"),

  saveSettings: (settings: AppSettings) =>
    invoke<AppSettings>("save_settings", { settings }),

  getPlatformStatus: () => invoke<PlatformStatus>("get_platform_status"),

  openAccessibility: () => invoke<void>("open_accessibility"),

  setSuspendBlurHide: (suspend: boolean) =>
    invoke<void>("set_suspend_blur_hide", { suspend }),

  openMainFromDock: () => invoke<void>("open_main_from_dock"),

  hideMainWindow: () => invoke<void>("hide_main_window"),
};
