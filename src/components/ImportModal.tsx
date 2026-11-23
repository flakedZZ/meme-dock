import { open } from "@tauri-apps/plugin-dialog";
import { ClipboardPaste, FolderOpen, Images, RefreshCw, X } from "lucide-react";
import { useEffect, useState } from "react";
import { api, fileUrl } from "../api";
import type { ImportResult, QqDirInfo, ScanCandidate } from "../types";

interface Props {
  onClose: () => void;
  onImported: (res: ImportResult) => void;
  quickCaptureShortcut: string;
}

export function ImportModal({
  onClose,
  onImported,
  quickCaptureShortcut,
}: Props) {
  const [tab, setTab] = useState<"qq" | "files" | "clipboard">("qq");
  const [qqDirs, setQqDirs] = useState<QqDirInfo[]>([]);
  const [selectedDir, setSelectedDir] = useState<string>("");
  const [candidates, setCandidates] = useState<ScanCandidate[]>([]);
  const [selectedPaths, setSelectedPaths] = useState<Set<string>>(new Set());
  const [batchTags, setBatchTags] = useState("");
  const [loading, setLoading] = useState(false);
  const [statusMsg, setStatusMsg] = useState("");

  useEffect(() => {
    api.discoverQqDirs().then((dirs) => {
      setQqDirs(dirs);
      if (dirs.length > 0) {
        setSelectedDir(dirs[0].path);
      }
    });
  }, []);

  const parseTags = () =>
    batchTags
      .split(/[,，\s]+/)
      .map((t) => t.trim())
      .filter(Boolean);

  const handleScanDir = async (dirPath: string) => {
    if (!dirPath) return;
    setLoading(true);
    setStatusMsg("正在按文件头魔数扫描表情...");
    try {
      const items = await api.scanQqDir(dirPath, 240);
      setCandidates(items);
      const fresh = new Set(
        items.filter((i) => !i.alreadyImported).map((i) => i.path),
      );
      setSelectedPaths(fresh);
      setStatusMsg(
        `扫描到 ${items.length} 张图片，其中 ${fresh.size} 张为未入库新表情`,
      );
    } catch (e) {
      setStatusMsg(`扫描失败: ${String(e)}`);
    } finally {
      setLoading(false);
    }
  };

  const handlePickManualQqDir = async () => {
    const picked = await open({ directory: true, multiple: false });
    if (typeof picked === "string") {
      setSelectedDir(picked);
      await handleScanDir(picked);
    }
  };

  const handleImportSelectedCandidates = async () => {
    const paths = Array.from(selectedPaths);
    if (paths.length === 0) return;
    setLoading(true);
    try {
      const res = await api.importPaths(paths, parseTags());
      onImported(res);
      onClose();
    } catch (e) {
      setStatusMsg(`导入失败: ${String(e)}`);
    } finally {
      setLoading(false);
    }
  };

  const handlePickFiles = async () => {
    const picked = await open({
      multiple: true,
      filters: [
        { name: "图片表情", extensions: ["png", "jpg", "jpeg", "gif", "webp"] },
      ],
    });
    if (!picked) return;
    const paths = Array.isArray(picked) ? picked : [picked];
    setLoading(true);
    try {
      const res = await api.importPaths(paths, parseTags());
      onImported(res);
      onClose();
    } finally {
      setLoading(false);
    }
  };

  const handlePickFolder = async () => {
    const picked = await open({ directory: true, multiple: false });
    if (typeof picked !== "string") return;
    setLoading(true);
    try {
      const res = await api.importPaths([picked], parseTags());
      onImported(res);
      onClose();
    } finally {
      setLoading(false);
    }
  };

  const handleClipboardImport = async () => {
    setLoading(true);
    try {
      const res = await api.importFromClipboard(parseTags());
      onImported(res);
      if (res.imported > 0 || res.duplicates > 0) {
        onClose();
      } else if (res.errors.length > 0) {
        setStatusMsg(res.errors[0]);
      }
    } catch (e) {
      setStatusMsg(`剪贴板读取失败: ${String(e)}`);
    } finally {
      setLoading(false);
    }
  };

  const toggleCandidate = (path: string) => {
    setSelectedPaths((prev) => {
      const next = new Set(prev);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  };

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal-card" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <span>导入表情包</span>
          <button className="icon-btn" onClick={onClose}>
            <X size={15} />
          </button>
        </div>

        <div className="modal-body">
          <div className="tabs">
            <button
              className={`tab-btn ${tab === "qq" ? "active" : ""}`}
              onClick={() => setTab("qq")}
            >
              QQ 表情扫描
            </button>
            <button
              className={`tab-btn ${tab === "files" ? "active" : ""}`}
              onClick={() => setTab("files")}
            >
              文件 / 目录
            </button>
            <button
              className={`tab-btn ${tab === "clipboard" ? "active" : ""}`}
              onClick={() => setTab("clipboard")}
            >
              剪贴板导入
            </button>
          </div>

          <div className="field">
            <label>批量附加标签（可选，空格或逗号分隔）</label>
            <input
              className="text-input"
              placeholder="例如：搞笑 熊猫头 常用"
              value={batchTags}
              onChange={(e) => setBatchTags(e.target.value)}
            />
          </div>

          {tab === "qq" && (
            <>
              <div className="field">
                <label>自动发现的 QQ 表情目录</label>
                {qqDirs.length > 0 ? (
                  <div style={{ display: "flex", gap: 6 }}>
                    <select
                      className="text-input"
                      value={selectedDir}
                      onChange={(e) => setSelectedDir(e.target.value)}
                    >
                      {qqDirs.map((d) => (
                        <option key={d.path} value={d.path}>
                          {d.label} (~{d.fileCountHint} 文件)
                        </option>
                      ))}
                    </select>
                    <button
                      className="btn btn-primary"
                      disabled={loading || !selectedDir}
                      onClick={() => handleScanDir(selectedDir)}
                    >
                      <RefreshCw size={13} />
                      扫描
                    </button>
                  </div>
                ) : (
                  <div className="switch-desc">
                    未自动检测到 QQ NT 表情目录，可手动选择目录扫描。
                  </div>
                )}
              </div>

              <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                <button
                  className="btn btn-secondary"
                  disabled={loading}
                  onClick={handlePickManualQqDir}
                >
                  <FolderOpen size={13} />
                  手动选择目录扫描
                </button>
                {candidates.length > 0 && (
                  <button
                    className="btn btn-secondary"
                    onClick={() => {
                      const fresh = candidates
                        .filter((c) => !c.alreadyImported)
                        .map((c) => c.path);
                      setSelectedPaths(
                        selectedPaths.size === fresh.length
                          ? new Set()
                          : new Set(fresh),
                      );
                    }}
                  >
                    全选/反选未入库
                  </button>
                )}
              </div>

              {statusMsg && <div className="switch-desc">{statusMsg}</div>}

              {candidates.length > 0 && (
                <div className="scan-grid">
                  {candidates.map((c) => {
                    const checked = selectedPaths.has(c.path);
                    return (
                      <div
                        key={c.sha256}
                        className={`scan-item ${checked ? "selected" : ""} ${
                          c.alreadyImported ? "imported" : ""
                        }`}
                        title={
                          c.alreadyImported
                            ? "已存在于表情库"
                            : `${c.width}×${c.height} ${c.ext.toUpperCase()}`
                        }
                        onClick={() => {
                          if (!c.alreadyImported) toggleCandidate(c.path);
                        }}
                      >
                        <img src={fileUrl(c.path)} loading="lazy" alt="" />
                        {c.isAnimated && <span className="gif-badge">GIF</span>}
                        {checked && <span className="scan-check">✓</span>}
                      </div>
                    );
                  })}
                </div>
              )}
            </>
          )}

          {tab === "files" && (
            <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
              <div style={{ display: "flex", gap: 8 }}>
                <button
                  className="btn btn-primary"
                  disabled={loading}
                  onClick={handlePickFiles}
                >
                  <Images size={14} />
                  选择多张图片
                </button>
                <button
                  className="btn btn-secondary"
                  disabled={loading}
                  onClick={handlePickFolder}
                >
                  <FolderOpen size={14} />
                  导入整个文件夹
                </button>
              </div>
              <div className="switch-desc">
                提示：你也可以直接将图片文件或文件夹拖拽到 MemeDock 主窗口内自动入库，支持
                PNG / JPG / GIF / WebP 并自动 SHA-256 去重。
              </div>
            </div>
          )}

          {tab === "clipboard" && (
            <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
              <button
                className="btn btn-primary"
                disabled={loading}
                onClick={handleClipboardImport}
              >
                <ClipboardPaste size={14} />
                立即读取剪贴板导入
              </button>
              <div className="switch-desc">
                支持直接读取复制的图片文件（保留 GIF 动图）或截图位图。平时也可以随时按全局快捷键{" "}
                <strong>{quickCaptureShortcut}</strong> 一键快收剪贴板表情。
              </div>
              {statusMsg && <div className="switch-desc">{statusMsg}</div>}
            </div>
          )}
        </div>

        <div className="modal-footer">
          <button className="btn btn-secondary" onClick={onClose}>
            关闭
          </button>
          {tab === "qq" && candidates.length > 0 && (
            <button
              className="btn btn-primary"
              disabled={loading || selectedPaths.size === 0}
              onClick={handleImportSelectedCandidates}
            >
              导入选中 ({selectedPaths.size})
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
