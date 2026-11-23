import { Trash2, X } from "lucide-react";
import { useState } from "react";
import { api, fileUrl, formatBytes } from "../api";
import type { MemeItem } from "../types";

interface Props {
  meme: MemeItem;
  onClose: () => void;
  onUpdated: (item: MemeItem) => void;
  onDeleted: (id: number) => void;
}

export function EditMemeModal({ meme, onClose, onUpdated, onDeleted }: Props) {
  const [alias, setAlias] = useState(meme.alias);
  const [tagsText, setTagsText] = useState(meme.tags.join(" "));
  const [saving, setSaving] = useState(false);

  const handleSave = async () => {
    setSaving(true);
    try {
      const tags = tagsText
        .split(/[,，\s]+/)
        .map((t) => t.trim())
        .filter(Boolean);
      const updated = await api.updateMemeMeta(meme.id, alias, tags);
      onUpdated(updated);
      onClose();
    } finally {
      setSaving(false);
    }
  };

  const handleDelete = async () => {
    setSaving(true);
    try {
      await api.deleteMeme(meme.id);
      onDeleted(meme.id);
      onClose();
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal-card" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <span>编辑表情信息</span>
          <button className="icon-btn" onClick={onClose}>
            <X size={15} />
          </button>
        </div>

        <div className="modal-body">
          <div style={{ display: "flex", gap: 12, alignItems: "center" }}>
            <img
              src={fileUrl(meme.originalPath)}
              alt=""
              style={{
                width: 84,
                height: 84,
                objectFit: "contain",
                borderRadius: 8,
                background: "var(--bg-subtle)",
                padding: 4,
              }}
            />
            <div className="switch-desc" style={{ lineHeight: 1.7 }}>
              <div>
                尺寸：{meme.width}×{meme.height} · {meme.ext.toUpperCase()}
                {meme.isAnimated ? " (动图)" : ""}
              </div>
              <div>大小：{formatBytes(meme.fileSize)}</div>
              <div>使用次数：{meme.useCount}</div>
            </div>
          </div>

          <div className="field">
            <label>别名（支持中文、拼音全拼及首字母搜索）</label>
            <input
              className="text-input"
              autoFocus
              placeholder="例如：熊猫头震惊"
              value={alias}
              onChange={(e) => setAlias(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") handleSave();
              }}
            />
          </div>

          <div className="field">
            <label>标签（空格或逗号分隔）</label>
            <input
              className="text-input"
              placeholder="例如：搞笑 熊猫头 震惊"
              value={tagsText}
              onChange={(e) => setTagsText(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") handleSave();
              }}
            />
          </div>
        </div>

        <div className="modal-footer" style={{ justifyContent: "space-between" }}>
          <button
            className="btn btn-danger"
            disabled={saving}
            onClick={handleDelete}
          >
            <Trash2 size={13} />
            删除表情
          </button>
          <div style={{ display: "flex", gap: 8 }}>
            <button className="btn btn-secondary" onClick={onClose}>
              取消
            </button>
            <button
              className="btn btn-primary"
              disabled={saving}
              onClick={handleSave}
            >
              保存
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
