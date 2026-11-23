import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { fileUrl, formatBytes } from "../api";
import type { PreviewPayload } from "../types";

export function PreviewWindow() {
  const [payload, setPayload] = useState<PreviewPayload | null>(null);

  useEffect(() => {
    document.documentElement.classList.add("transparent-window");
    const unlistenPromise = listen<PreviewPayload>("preview-update", (event) => {
      setPayload(event.payload);
    });
    return () => {
      unlistenPromise.then((fn) => fn());
    };
  }, []);

  if (!payload) {
    return null;
  }

  return (
    <div className="preview-card">
      <div className="preview-inner">
        <div className="preview-media">
          <img
            key={payload.originalPath}
            src={fileUrl(payload.originalPath)}
            alt={payload.alias || "meme preview"}
          />
        </div>
        <div className="preview-meta">
          <div className="preview-title">
            {payload.alias || `表情 #${payload.id}`}
          </div>
          <div className="preview-sub">
            <span>
              {payload.width}×{payload.height} · {payload.ext.toUpperCase()}
              {payload.isAnimated ? " 动图" : ""}
            </span>
            <span>{formatBytes(payload.fileSize)}</span>
          </div>
          {payload.tags.length > 0 && (
            <div className="preview-tags">
              {payload.tags.slice(0, 5).map((tag) => (
                <span key={tag} className="tag-pill">
                  #{tag}
                </span>
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
