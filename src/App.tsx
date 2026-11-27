import { useCallback, useEffect, useMemo, useState } from "react";
import { api, errorText, listen } from "./lib/api";
import { AppContext, type View } from "./lib/store";
import type { Bootstrap, Client, NotificationItem, Project, TimerSnapshot } from "./lib/types";
import { Icon, type IconName } from "./components/Icon";
import { fmtClock } from "./lib/format";
import Dashboard from "./views/Dashboard";
import Entries from "./views/Entries";
import Clients from "./views/Clients";
import Projects from "./views/Projects";
import Reports from "./views/Reports";
import Payments from "./views/Payments";
import Notifications from "./views/Notifications";
import Profiles from "./views/Profiles";
import SettingsView from "./views/Settings";

const NAV: { view: View; label: string; icon: IconName; group: string }[] = [
  { view: "dashboard", label: "לוח בקרה", icon: "grid", group: "עבודה" },
  { view: "entries", label: "יומן עבודה", icon: "clock", group: "עבודה" },
  { view: "clients", label: "לקוחות", icon: "users", group: "ניהול" },
  { view: "projects", label: "פרויקטים", icon: "folder", group: "ניהול" },
  { view: "reports", label: "דוחות וסיכומים", icon: "chart", group: "כספים" },
  { view: "payments", label: "תשלומים", icon: "wallet", group: "כספים" },
  { view: "notifications", label: "התראות", icon: "bell", group: "מערכת" },
  { view: "profiles", label: "פרופילים", icon: "user", group: "מערכת" },
  { view: "settings", label: "הגדרות", icon: "settings", group: "מערכת" },
];

const TITLES: Record<View, { title: string; sub: string }> = {
  dashboard: { title: "לוח בקרה", sub: "תמונת מצב של היום והשבוע" },
  entries: { title: "יומן עבודה", sub: "כל רשומות הזמן, כולל הוספה ועריכה ידנית" },
  clients: { title: "לקוחות", sub: "אנשי קשר, תעריפים וחובות פתוחים" },
  projects: { title: "פרויקטים", sub: "ניהול פרויקטים, תעריפים ותקציבים" },
  reports: { title: "דוחות וסיכומים", sub: "פילוח לפי יום, לקוח ופרויקט" },
  payments: { title: "תשלומים", sub: "תיעוד הכנסות וסגירת חיובים" },
  notifications: { title: "התראות", sub: "הודעות מערכת ותזכורות עבודה" },
  profiles: { title: "פרופילים", sub: "הפרדה בין מקומות עבודה או עסקים" },
  settings: { title: "הגדרות", sub: "התאמה אישית, גיבוי ועדכונים" },
};

export default function App() {
  const [boot, setBoot] = useState<Bootstrap | null>(null);
  const [timer, setTimer] = useState<TimerSnapshot | null>(null);
  const [projects, setProjects] = useState<Project[]>([]);
  const [clients, setClients] = useState<Client[]>([]);
  const [notifications, setNotifications] = useState<NotificationItem[]>([]);
  const [view, setView] = useState<View>("dashboard");
  const [reloadKey, setReloadKey] = useState(0);
  const [toast, setToast] = useState<{ text: string; tone: "ok" | "error" } | null>(null);
  const [fatal, setFatal] = useState<string | null>(null);

  const notify = useCallback((text: string, tone: "ok" | "error" = "ok") => {
    setToast({ text, tone });
  }, []);

  const refreshAll = useCallback(async () => {
    try {
      const next = await api.bootstrap();
      setBoot(next);
      setTimer(next.timer);
      const [projectList, clientList, notificationList] = await Promise.all([
        api.projects(),
        api.clients(),
        api.notifications(),
      ]);
      setProjects(projectList);
      setClients(clientList);
      setNotifications(notificationList);
      setReloadKey((key) => key + 1);
    } catch (error) {
      const text = errorText(error);
      if (!boot) setFatal(text);
      notify(text, "error");
    }
  }, [boot, notify]);

  useEffect(() => {
    void refreshAll();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    const unlisteners: Array<() => void> = [];
    const wire = async () => {
      unlisteners.push(await listen<TimerSnapshot>("tick", (event) => setTimer(event.payload)));
      unlisteners.push(await listen("state-changed", () => void refreshAll()));
      unlisteners.push(
        await listen<string>("navigate", (event) => setView(event.payload as View))
      );
      unlisteners.push(await listen("settings-changed", () => void refreshAll()));
    };
    void wire();
    return () => unlisteners.forEach((off) => off());
  }, [refreshAll]);

  useEffect(() => {
    if (!toast) return;
    const handle = window.setTimeout(() => setToast(null), 3400);
    return () => window.clearTimeout(handle);
  }, [toast]);

  const theme = boot?.settings.theme ?? "dark";
  useEffect(() => {
    const media = window.matchMedia("(prefers-color-scheme: light)");
    const apply = () => {
      const resolved = theme === "system" ? (media.matches ? "light" : "dark") : theme;
      document.documentElement.dataset.theme = resolved;
    };
    apply();
    media.addEventListener("change", apply);
    return () => media.removeEventListener("change", apply);
  }, [theme]);

  const store = useMemo(
    () =>
      boot && timer
        ? {
            version: boot.version,
            settings: boot.settings,
            currency: boot.settings.currency ?? "ILS",
            profiles: boot.profiles,
            activeProfileId: boot.activeProfileId,
            timer,
            projects,
            clients,
            notifications,
            reloadKey,
            refreshAll,
            notify,
            navigate: setView,
          }
        : null,
    [boot, timer, projects, clients, notifications, reloadKey, refreshAll, notify]
  );

  if (fatal) {
    return (
      <div className="empty" style={{ height: "100vh", justifyContent: "center" }}>
        <span className="empty-icon">
          <Icon name="info" size={22} />
        </span>
        <strong>לא ניתן לטעון את הנתונים</strong>
        <p>{fatal}</p>
      </div>
    );
  }

  if (!store) {
    return (
      <div className="empty" style={{ height: "100vh", justifyContent: "center" }}>
        <span className="spinner" style={{ width: 22, height: 22 }} />
        <strong>טוען את WorkTime…</strong>
      </div>
    );
  }

  const activeProfile = store.profiles.find((profile) => profile.isActive);
  const unread = notifications.filter((item) => !item.read).length;
  const groups = [...new Set(NAV.map((item) => item.group))];
  const heading = TITLES[view];

  return (
    <AppContext.Provider value={store}>
      <div className="app">
        <aside className="sidebar">
          <div className="brand">
            <span className="brand-mark">
              <Icon name="clock" size={20} strokeWidth={2} />
            </span>
            <span className="brand-copy">
              <span className="brand-name">WorkTime</span>
              <span className="brand-version">גרסה {store.version}</span>
            </span>
          </div>

          {groups.map((group) => (
            <div key={group}>
              <div className="nav-group-label">{group}</div>
              {NAV.filter((item) => item.group === group).map((item) => (
                <button
                  key={item.view}
                  type="button"
                  className={`nav-item ${view === item.view ? "is-active" : ""}`}
                  onClick={() => setView(item.view)}
                >
                  <Icon name={item.icon} size={17} />
                  <span className="nav-text">{item.label}</span>
                  {item.view === "notifications" && unread > 0 && (
                    <span className="nav-count">{unread}</span>
                  )}
                </button>
              ))}
            </div>
          ))}

          <div className="sidebar-foot">
            <span>
              מחובר כ־<strong>{activeProfile?.name ?? "—"}</strong>
            </span>
            <span>התוכנה ממשיכה לרוץ ברקע. אפשר לסגור את החלון ולשלוט בה משורת המשימות.</span>
            <button
              type="button"
              className="btn btn-ghost btn-sm"
              onClick={() => void api.quit()}
              style={{ justifyContent: "flex-start", paddingInline: 0 }}
            >
              <Icon name="close" size={14} /> יציאה מהתוכנה
            </button>
          </div>
        </aside>

        <div className="main">
          <header className="topbar">
            <div className="topbar-title">
              <strong>{heading.title}</strong>
              <span>{heading.sub}</span>
            </div>

            <button
              type="button"
              className={`timer-chip ${timer?.running ? "is-running" : ""}`}
              onClick={() => setView("dashboard")}
              title="מעבר לטיימר"
            >
              <span className="dot" />
              <strong>{fmtClock(timer?.elapsedSec ?? 0)}</strong>
              <span className="muted small">
                {timer?.running ? timer.projectName ?? "ללא פרויקט" : "לא פעיל"}
              </span>
            </button>

            <button
              type="button"
              className="icon-btn"
              title="שעון צף"
              aria-label="שעון צף"
              onClick={async () => {
                try {
                  await api.setFloating(!(boot?.floatingVisible ?? false));
                  await refreshAll();
                } catch (error) {
                  notify(errorText(error), "error");
                }
              }}
            >
              <Icon name="pin" size={17} />
            </button>

            <button
              type="button"
              className="icon-btn"
              title="התראות"
              aria-label="התראות"
              onClick={() => setView("notifications")}
            >
              <Icon name="bell" size={17} />
            </button>

            <select
              className="input select"
              style={{ width: 150 }}
              value={store.activeProfileId}
              onChange={async (event) => {
                try {
                  await api.switchProfile(event.target.value);
                  await refreshAll();
                  notify("הפרופיל הוחלף");
                } catch (error) {
                  notify(errorText(error), "error");
                }
              }}
            >
              {store.profiles.map((profile) => (
                <option key={profile.id} value={profile.id}>
                  {profile.name}
                </option>
              ))}
            </select>
          </header>

          <div className="content">
            {view === "dashboard" && <Dashboard />}
            {view === "entries" && <Entries />}
            {view === "clients" && <Clients />}
            {view === "projects" && <Projects />}
            {view === "reports" && <Reports />}
            {view === "payments" && <Payments />}
            {view === "notifications" && <Notifications />}
            {view === "profiles" && <Profiles />}
            {view === "settings" && <SettingsView />}
          </div>
        </div>

        {toast && <div className={`toast ${toast.tone === "error" ? "is-error" : "is-ok"}`}>{toast.text}</div>}
      </div>
    </AppContext.Provider>
  );
}
