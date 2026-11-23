import { Sparkles } from "lucide-react";
import { useEffect } from "react";
import { api } from "../api";

export function DockWindow() {
  useEffect(() => {
    document.documentElement.classList.add("transparent-window");
  }, []);

  return (
    <button
      className="dock-pill"
      title="打开 MemeDock 表情面板"
      onClick={() => api.openMainFromDock()}
    >
      <Sparkles size={15} />
      <span>表情</span>
    </button>
  );
}
