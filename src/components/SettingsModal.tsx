import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
import { ShieldAlert, ShieldCheck, X } from "lucide-react";
import { useEffect, useState } from "react";
import { api } from "../api";
import type { AppSettings, PlatformStatus } from "../types";

interface Props {
  settings: AppSettings;
  onClose: () => void;
  onSaved: (next: AppSettings) => void;
}

export function SettingsModal({ settings, onClose, onSaved }: Props) {
  const [form, setForm] = useState<AppSettings>(settings);
  const [platform, setPlatform] = useState<PlatformStatus | null>(null);
  const [autostartOn, setAutostartOn] = useState(false);
  const [error, setError] = useState("");
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    api.getPlatformStatus().then(setPlatform);
    isEnabled()
      .then(setAutostartOn)
      .catch(() => {});
  }, []);

  const handleToggleAutostart = async (checked: boolean) => {
    try {
      if (checked) await enable();
      else await disable();
      setAutostartOn(checked);
    } catch (e) {
      setError(`开机自启设置失败: ${String(e)}`);
    }
  };

  const handleSave = async () => {
    setSaving(true);
    setError("");
    try {
      const saved = await api.saveSettings(form);
      onSaved(saved);
      onClose();
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal-card" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <span>设置</span>
          <button className="icon-btn" onClick={onClose}>
            <X size={15} />
          </button>
        </div>

        <div className="modal-body">
          <div className="field">
            <label>全局召出快捷键（默认 Alt+E / Option+E）</label>
            <input
              className="text-input"
              value={form.toggleShortcut}
              onChange={(e) =>
                setForm({ ...form, toggleShortcut: e.target.value })
              }
            />
          </div>

          <div className="field">
            <label>剪贴板快收全局快捷键（默认 Alt+Shift+E）</label>
            <input
              className="text-input"
              value={form.quickCaptureShortcut}
              onChange={(e) =>
                setForm({ ...form, quickCaptureShortcut: e.target.value })
              }
            />
          </div>

          <div className="switch-row">
            <div>
              <div>粘贴后自动回车发送</div>
              <div className="switch-desc">
                默认关闭（仅粘贴到 QQ 输入框）；开启后将自动按回车发送
              </div>
            </div>
            <input
              type="checkbox"
              checked={form.autoSend}
              onChange={(e) => setForm({ ...form, autoSend: e.target.checked })}
            />
          </div>

          <div className="switch-row">
            <div>
              <div>QQ 窗口边缘贴靠按钮</div>
              <div className="switch-desc">
                仅在 QQ 处于前台时低频追踪窗口右侧，显示快捷悬浮按钮
              </div>
            </div>
            <input
              type="checkbox"
              checked={form.qqDockEnabled}
              onChange={(e) =>
                setForm({ ...form, qqDockEnabled: e.target.checked })
              }
            />
          </div>

          <div className="switch-row">
            <div>
              <div>开机自动启动</div>
              <div className="switch-desc">后台托盘静默常驻，低内存占用</div>
            </div>
            <input
              type="checkbox"
              checked={autostartOn}
              onChange={(e) => handleToggleAutostart(e.target.checked)}
            />
          </div>

          <div style={{ display: "flex", gap: 10 }}>
            <div className="field" style={{ flex: 1 }}>
              <label>主题外观</label>
              <select
                className="text-input"
                value={form.theme}
                onChange={(e) => setForm({ ...form, theme: e.target.value })}
              >
                <option value="system">跟随系统</option>
                <option value="light">浅色模式</option>
                <option value="dark">深色模式</option>
              </select>
            </div>
            <div className="field" style={{ flex: 1 }}>
              <label>粘贴延迟 (ms)</label>
              <input
                type="number"
                className="text-input"
                min={30}
                max={1000}
                value={form.pasteDelayMs}
                onChange={(e) =>
                  setForm({
                    ...form,
                    pasteDelayMs: Number(e.target.value) || 120,
                  })
                }
              />
            </div>
          </div>

          {platform?.os === "macos" && (
            <div
              className="switch-row"
              style={{
                background: "var(--bg-subtle)",
                padding: "8px 10px",
                borderRadius: 8,
              }}
            >
              <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                {platform.accessibilityGranted ? (
                  <ShieldCheck size={16} color="var(--success)" />
                ) : (
                  <ShieldAlert size={16} color="var(--danger)" />
                )}
                <div>
                  <div>
                    macOS 辅助功能权限：
                    {platform.accessibilityGranted ? "已授权" : "未授权"}
                  </div>
                  <div className="switch-desc">
                    自动模拟 Cmd+V 粘贴到 QQ 需要授予辅助功能权限
                  </div>
                </div>
              </div>
              {!platform.accessibilityGranted && (
                <button
                  className="btn btn-primary"
                  onClick={() => api.openAccessibility()}
                >
                  前往授权
                </button>
              )}
            </div>
          )}

          {platform && (
            <div className="switch-desc">数据目录：{platform.dataDir}</div>
          )}

          {error && (
            <div style={{ color: "var(--danger)", fontSize: 12 }}>{error}</div>
          )}
        </div>

        <div className="modal-footer">
          <button className="btn btn-secondary" onClick={onClose}>
            取消
          </button>
          <button
            className="btn btn-primary"
            disabled={saving}
            onClick={handleSave}
          >
            保存设置
          </button>
        </div>
      </div>
    </div>
  );
}
