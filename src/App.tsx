import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import {
  ChevronLeft,
  ChevronRight,
  ClipboardPaste,
  Edit3,
  FolderPlus,
  Pin,
  Search,
  Settings,
  Sparkles,
  Upload,
  X,
} from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { api, fileUrl } from "./api";
import { EditMemeModal } from "./components/EditMemeModal";
import { ImportModal } from "./components/ImportModal";
import { SettingsModal } from "./components/SettingsModal";
import type { AppSettings, ImportResult, MemeItem, MemePage, TagInfo } from "./types";

const DEFAULT_SETTINGS: AppSettings = {
  toggleShortcut: "Alt+E",
  quickCaptureShortcut: "Alt+Shift+E",
  autoSend: false,
  qqDockEnabled: false,
  pageSize: 20,
  theme: "system",
  pasteDelayMs: 120,
};

export function App() {
  const [query, setQuery] = useState("");
  const [selectedTags, setSelectedTags] = useState<string[]>([]);
  const [sort, setSort] = useState<"recent" | "frequent" | "newest">("recent");
  const [page, setPage] = useState(1);
  const [data, setData] = useState<MemePage>({
    items: [],
    total: 0,
    page: 1,
    pageSize: 20,
    totalPages: 1,
  });
  const [tags, setTags] = useState<TagInfo[]>([]);
  const [settings, setSettings] = useState<AppSettings>(DEFAULT_SETTINGS);
  const [focusedIdx, setFocusedIdx] = useState(0);

  const [showImport, setShowImport] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [editingMeme, setEditingMeme] = useState<MemeItem | null>(null);
  const [pinned, setPinned] = useState(false);
  const [dragOver, setDragOver] = useState(false);
  const [toast, setToast] = useState("");

  const searchRef = useRef<HTMLInputElement>(null);
  const hoverTimerRef = useRef<number | null>(null);
  const toastTimerRef = useRef<number | null>(null);
  const wheelLockRef = useRef(0);

  const flashToast = useCallback((msg: string) => {
    setToast(msg);
    if (toastTimerRef.current) window.clearTimeout(toastTimerRef.current);
    toastTimerRef.current = window.setTimeout(() => setToast(""), 2600);
  }, []);

  const applyTheme = useCallback((theme: string) => {
    if (theme === "dark" || theme === "light") {
      document.documentElement.setAttribute("data-theme", theme);
    } else {
      document.documentElement.removeAttribute("data-theme");
    }
  }, []);

  const refreshMemes = useCallback(async () => {
    try {
      const res = await api.queryMemes({
        query,
        tags: selectedTags,
        page,
        pageSize: settings.pageSize || 20,
        sort,
      });
      setData(res);
      setFocusedIdx(0);
    } catch (e) {
      flashToast(`加载失败: ${String(e)}`);
    }
  }, [query, selectedTags, page, settings.pageSize, sort, flashToast]);

  const refreshTags = useCallback(async () => {
    try {
      const list = await api.listTags();
      setTags(list);
    } catch {
      // ignore
    }
  }, []);

  useEffect(() => {
    api.getSettings().then((s) => {
      setSettings(s);
      applyTheme(s.theme);
    });
  }, [applyTheme]);

  useEffect(() => {
    refreshMemes();
  }, [refreshMemes]);

  useEffect(() => {
    refreshTags();
  }, [refreshTags]);

  // Suspend blur auto-hide when pinned or a modal is open
  useEffect(() => {
    api.setSuspendBlurHide(pinned || showImport || showSettings || !!editingMeme);
  }, [pinned, showImport, showSettings, editingMeme]);

  // Listen to backend events (focus search, global clipboard quick-capture)
  useEffect(() => {
    const unFocus = listen("focus-search", () => {
      window.setTimeout(() => {
        searchRef.current?.focus();
        searchRef.current?.select();
      }, 30);
    });
    const unClip = listen<ImportResult>("clipboard-imported", (e) => {
      const r = e.payload;
      if (r.imported > 0) {
        flashToast(`剪贴板快收成功：新增 ${r.imported} 张表情`);
        refreshMemes();
        refreshTags();
      } else if (r.duplicates > 0) {
        flashToast("剪贴板表情已存在于库中（已去重）");
      } else if (r.errors.length > 0) {
        flashToast(r.errors[0]);
      }
    });
    return () => {
      unFocus.then((f) => f());
      unClip.then((f) => f());
    };
  }, [flashToast, refreshMemes, refreshTags]);

  // Drag & drop files / folders directly onto the main window
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    getCurrentWebview()
      .onDragDropEvent(async (event) => {
        if (event.payload.type === "over") {
          setDragOver(true);
        } else if (event.payload.type === "leave") {
          setDragOver(false);
        } else if (event.payload.type === "drop") {
          setDragOver(false);
          const paths = event.payload.paths;
          if (paths.length > 0) {
            const res = await api.importPaths(paths, []);
            handleImportFinished(res);
          }
        }
      })
      .then((fn) => {
        unlisten = fn;
      });
    return () => {
      if (unlisten) unlisten();
    };
  });

  const handleImportFinished = (res: ImportResult) => {
    const parts: string[] = [];
    if (res.imported > 0) parts.push(`新增 ${res.imported} 张`);
    if (res.duplicates > 0) parts.push(`跳过重复 ${res.duplicates} 张`);
    if (res.skipped > 0) parts.push(`忽略非图片 ${res.skipped} 个`);
    flashToast(parts.length > 0 ? `导入完成：${parts.join("，")}` : "未导入新表情");
    setPage(1);
    refreshMemes();
    refreshTags();
  };

  const clearHoverPreview = useCallback(() => {
    if (hoverTimerRef.current) {
      window.clearTimeout(hoverTimerRef.current);
      hoverTimerRef.current = null;
    }
    api.hidePreview();
  }, []);

  const handleCardEnter = (id: number) => {
    if (hoverTimerRef.current) window.clearTimeout(hoverTimerRef.current);
    hoverTimerRef.current = window.setTimeout(() => {
      api.showPreview(id);
    }, 200);
  };

  const handlePaste = useCallback(
    async (meme: MemeItem, keepOpen: boolean) => {
      clearHoverPreview();
      try {
        const res = await api.pasteMeme(meme.id, keepOpen || pinned);
        if (!res.pasted || keepOpen || pinned) {
          flashToast(res.message);
        }
        refreshMemes();
      } catch (e) {
        flashToast(`粘贴失败: ${String(e)}`);
      }
    },
    [clearHoverPreview, flashToast, pinned, refreshMemes],
  );

  // Keyboard navigation & pagination
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (showImport || showSettings || editingMeme) {
        if (e.key === "Escape") {
          setShowImport(false);
          setShowSettings(false);
          setEditingMeme(null);
        }
        return;
      }

      if (e.key === "Escape") {
        clearHoverPreview();
        api.hideMainWindow();
        return;
      }

      const inSearch = document.activeElement === searchRef.current;
      if (e.key === "ArrowLeft" && (!inSearch || query === "")) {
        e.preventDefault();
        setPage((p) => Math.max(1, p - 1));
      } else if (e.key === "ArrowRight" && (!inSearch || query === "")) {
        e.preventDefault();
        setPage((p) => Math.min(data.totalPages, p + 1));
      } else if (e.key === "Enter" && data.items[focusedIdx]) {
        e.preventDefault();
        handlePaste(data.items[focusedIdx], e.shiftKey);
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [
    showImport,
    showSettings,
    editingMeme,
    query,
    data.items,
    data.totalPages,
    focusedIdx,
    clearHoverPreview,
    handlePaste,
  ]);

  const handleWheel = (e: React.WheelEvent) => {
    const now = Date.now();
    if (now - wheelLockRef.current < 180) return;
    if (Math.abs(e.deltaY) < 12) return;
    wheelLockRef.current = now;
    if (e.deltaY > 0 && page < data.totalPages) {
      setPage((p) => p + 1);
    } else if (e.deltaY < 0 && page > 1) {
      setPage((p) => p - 1);
    }
  };

  const toggleTagFilter = (tagName: string) => {
    setPage(1);
    setSelectedTags((prev) =>
      prev.includes(tagName)
        ? prev.filter((t) => t !== tagName)
        : [...prev, tagName],
    );
  };

  const handleQuickClipboard = async () => {
    try {
      const res = await api.importFromClipboard([]);
      if (res.imported > 0 || res.duplicates > 0) {
        handleImportFinished(res);
      } else if (res.errors.length > 0) {
        flashToast(res.errors[0]);
      }
    } catch (e) {
      flashToast(`剪贴板导入失败: ${String(e)}`);
    }
  };

  return (
    <div className="app-shell">
      {/* Top draggable search bar */}
      <div className="topbar" data-tauri-drag-region>
        <div className="brand-drag" data-tauri-drag-region title="拖动窗口">
          <Sparkles size={14} />
          <span>MemeDock</span>
        </div>

        <div className="search-wrap">
          <Search size={13} color="var(--text-muted)" />
          <input
            ref={searchRef}
            className="search-input"
            placeholder="搜索别名 / 拼音首字母 / 标签..."
            value={query}
            onChange={(e) => {
              setQuery(e.target.value);
              setPage(1);
            }}
          />
          {query && (
            <button
              className="icon-btn"
              style={{ width: 18, height: 18 }}
              onClick={() => {
                setQuery("");
                setPage(1);
              }}
            >
              <X size={12} />
            </button>
          )}
        </div>

        <button
          className="icon-btn"
          title={`剪贴板快收 (${settings.quickCaptureShortcut})`}
          onClick={handleQuickClipboard}
        >
          <ClipboardPaste size={15} />
        </button>
        <button
          className="icon-btn"
          title="导入表情包 (QQ扫描 / 文件 / 剪贴板)"
          onClick={() => setShowImport(true)}
        >
          <FolderPlus size={15} />
        </button>
        <button
          className={`icon-btn ${pinned ? "active" : ""}`}
          title={pinned ? "已固定窗口（点击取消）" : "固定窗口不自动隐藏"}
          onClick={() => setPinned((p) => !p)}
        >
          <Pin size={15} />
        </button>
        <button
          className="icon-btn"
          title="设置"
          onClick={() => setShowSettings(true)}
        >
          <Settings size={15} />
        </button>
        <button
          className="icon-btn"
          title="隐藏到托盘 (Esc)"
          onClick={() => {
            clearHoverPreview();
            api.hideMainWindow();
          }}
        >
          <X size={15} />
        </button>
      </div>

      {/* Sort & Multi-tag Filter Bar */}
      <div className="tag-bar">
        <select
          className="sort-select"
          value={sort}
          onChange={(e) => {
            setSort(e.target.value as "recent" | "frequent" | "newest");
            setPage(1);
          }}
        >
          <option value="recent">最近使用</option>
          <option value="frequent">最常使用</option>
          <option value="newest">最新导入</option>
        </select>

        {selectedTags.length > 0 && (
          <button
            className="tag-pill"
            onClick={() => {
              setSelectedTags([]);
              setPage(1);
            }}
          >
            清除筛选 ×
          </button>
        )}

        {tags.map((t) => (
          <button
            key={t.id}
            className={`tag-pill ${
              selectedTags.includes(t.name) ? "selected" : ""
            }`}
            onClick={() => toggleTagFilter(t.name)}
          >
            #{t.name} ({t.count})
          </button>
        ))}
      </div>

      {/* 5x4 Grid */}
      <div
        className="grid-area"
        onWheel={handleWheel}
        onMouseLeave={clearHoverPreview}
      >
        {data.items.length > 0 ? (
          <div className="meme-grid">
            {data.items.map((item, idx) => (
              <div
                key={item.id}
                className={`meme-card ${idx === focusedIdx ? "focused" : ""}`}
                onMouseEnter={() => {
                  setFocusedIdx(idx);
                  handleCardEnter(item.id);
                }}
                onMouseLeave={clearHoverPreview}
                onClick={(e) => handlePaste(item, e.shiftKey)}
                onContextMenu={(e) => {
                  e.preventDefault();
                  clearHoverPreview();
                  setEditingMeme(item);
                }}
              >
                <img src={fileUrl(item.thumbPath)} alt={item.alias} />
                {item.isAnimated && <span className="gif-badge">GIF</span>}
                <button
                  className="card-edit-btn"
                  title="编辑别名和标签"
                  onClick={(e) => {
                    e.stopPropagation();
                    clearHoverPreview();
                    setEditingMeme(item);
                  }}
                >
                  <Edit3 size={11} />
                </button>
                {item.alias && <div className="card-alias">{item.alias}</div>}
              </div>
            ))}
          </div>
        ) : (
          <div className="empty-state">
            <Upload size={28} />
            <div>
              {query || selectedTags.length > 0
                ? "没有找到匹配的表情包"
                : "表情库还是空的，拖入图片或点击下方快速导入"}
            </div>
            <div className="empty-actions">
              <button
                className="btn btn-primary"
                onClick={() => setShowImport(true)}
              >
                <FolderPlus size={14} />
                扫描 QQ 表情 / 选择文件
              </button>
              <button
                className="btn btn-secondary"
                onClick={handleQuickClipboard}
              >
                <ClipboardPaste size={14} />
                从剪贴板导入
              </button>
            </div>
          </div>
        )}

        {dragOver && (
          <div className="drag-overlay">
            <Upload size={32} />
            <span>松开鼠标导入图片或文件夹</span>
          </div>
        )}
      </div>

      {/* Footer */}
      <div className="footer">
        <span>
          共 {data.total} 个 · 点击粘贴 · Shift+点击连续粘贴 · 右键编辑
        </span>
        <div className="pager">
          <button
            className="pager-btn"
            disabled={page <= 1}
            onClick={() => setPage((p) => Math.max(1, p - 1))}
          >
            <ChevronLeft size={12} />
          </button>
          <span>
            {data.page} / {data.totalPages}
          </span>
          <button
            className="pager-btn"
            disabled={page >= data.totalPages}
            onClick={() => setPage((p) => Math.min(data.totalPages, p + 1))}
          >
            <ChevronRight size={12} />
          </button>
        </div>
      </div>

      {toast && <div className="toast">{toast}</div>}

      {showImport && (
        <ImportModal
          onClose={() => setShowImport(false)}
          onImported={handleImportFinished}
          quickCaptureShortcut={settings.quickCaptureShortcut}
        />
      )}

      {showSettings && (
        <SettingsModal
          settings={settings}
          onClose={() => setShowSettings(false)}
          onSaved={(next) => {
            setSettings(next);
            applyTheme(next.theme);
            flashToast("设置已保存");
          }}
        />
      )}

      {editingMeme && (
        <EditMemeModal
          meme={editingMeme}
          onClose={() => setEditingMeme(null)}
          onUpdated={() => {
            flashToast("表情信息已更新");
            refreshMemes();
            refreshTags();
          }}
          onDeleted={() => {
            flashToast("表情已删除");
            refreshMemes();
            refreshTags();
          }}
        />
      )}
    </div>
  );
}
