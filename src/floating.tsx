import { useCallback, useEffect, useState } from "react";
import ReactDOM from "react-dom/client";
import { api, listen } from "./lib/api";
import { Icon } from "./components/Icon";
import { fmtClock, fmtMoney } from "./lib/format";
import { settingBoolean, settingNumber } from "./lib/store";
import type { Project, Settings, TimerSnapshot } from "./lib/types";
import "./styles.css";

function Floating() {
  const [timer, setTimer] = useState<TimerSnapshot | null>(null);
  const [settings, setSettings] = useState<Settings>({});
  const [projects, setProjects] = useState<Project[]>([]);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    try {
      const boot = await api.bootstrap();
      setTimer(boot.timer);
      setSettings(boot.settings);
      setProjects(await api.projects());
    } catch {
      /* ignore transient errors while the backend restarts */
    }
  }, []);

  useEffect(() => {
    document.body.style.background = "transparent";
    document.body.style.overflow = "hidden";
    void load();
    const unlisteners: Array<() => void> = [];
    void (async () => {
      unlisteners.push(await listen<TimerSnapshot>("tick", (event) => setTimer(event.payload)));
      unlisteners.push(await listen("state-changed", () => void load()));
      unlisteners.push(await listen("settings-changed", () => void load()));
      unlisteners.push(await listen("floating-options", () => void load()));
    })();
    return () => unlisteners.forEach((off) => off());
  }, [load]);

  const running = timer?.running ?? false;
  const showSeconds = settingBoolean(settings, "floatingSeconds", true);
  const compact = settingBoolean(settings, "floatingCompact", false);
  const opacity = settingNumber(settings, "floatingOpacity", 0.94);

  const toggle = async () => {
    setBusy(true);
    try {
      if (running) {
        await api.stopTimer();
      } else {
        await api.startTimer(projects.find((project) => !project.archived)?.id ?? null);
      }
      await load();
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className={`floating ${compact ? "floating-compact" : ""}`} style={{ opacity }}>
      <div className="floating-grip" data-tauri-drag-region />
      <div
        className="floating-body"
        data-tauri-drag-region
        onClick={() => void api.showMain("dashboard")}
        title="פתיחת WorkTime"
      >
        <div className={`floating-time ${running ? "is-running" : ""}`}>
          {fmtClock(timer?.elapsedSec ?? 0, showSeconds)}
        </div>
        <div className="floating-meta">
          {running
            ? `${timer?.projectName ?? "ללא פרויקט"}${timer?.rate ? ` · ${fmtMoney(timer.rate)}/ש׳` : ""}`
            : "לא פעיל · לחצו להתחלה"}
        </div>
      </div>
      <button
        type="button"
        className={`floating-btn ${running ? "is-running" : ""}`}
        onClick={() => void toggle()}
        disabled={busy}
        aria-label={running ? "עצור" : "התחל"}
        title={running ? "עצור וסכם" : "התחל עבודה"}
      >
        <Icon name={running ? "stop" : "play"} size={14} />
      </button>
      <button
        type="button"
        className="floating-btn"
        onClick={async () => {
          try {
            await api.setFloating(false);
          } catch {
            /* ignore */
          }
        }}
        aria-label="הסתר"
        title="הסתרת השעון הצף"
      >
        <Icon name="close" size={13} />
      </button>
    </div>
  );
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(<Floating />);
