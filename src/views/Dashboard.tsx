import { useEffect, useMemo, useState } from "react";
import { api, errorText } from "../lib/api";
import { settingNumber, useApp } from "../lib/store";
import type { Dashboard as DashboardData, Entry } from "../lib/types";
import { Badge, BarChart, Button, Card, Empty, ProgressRing, Stat } from "../components/ui";
import { Icon } from "../components/Icon";
import { fmtClock, fmtHM, fmtMoney, fmtTime, relativeDay } from "../lib/format";

export default function Dashboard() {
  const { settings, currency, timer, projects, navigate, notify, reloadKey } = useApp();
  const [data, setData] = useState<DashboardData | null>(null);
  const [recent, setRecent] = useState<Entry[]>([]);
  const [projectId, setProjectId] = useState("");
  const [description, setDescription] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let alive = true;
    Promise.all([api.dashboard(), api.entries({ limit: 6 })])
      .then(([dashboardData, entries]) => {
        if (!alive) return;
        setData(dashboardData);
        setRecent(entries);
      })
      .catch((error) => notify(errorText(error), "error"));
    return () => {
      alive = false;
    };
  }, [reloadKey, notify]);

  useEffect(() => {
    if (timer.active) {
      setProjectId(timer.active.projectId ?? "");
      setDescription(timer.active.description);
    }
  }, [timer.active?.id]);

  const goalHours = settingNumber(settings, "dailyGoalHours", 8);
  const workday = settingNumber(settings, "workdayHours", 8.5);
  const goalSeconds = goalHours * 3600;
  const todaySeconds = data?.todaySeconds ?? 0;
  // The backend prices the running entry the way stopping will (rounding included), so
  // show its amount instead of recomputing the raw elapsed time here.
  const liveEarnings = timer.active?.amount ?? 0;

  const weekData = useMemo(
    () => (data?.weekDays ?? []).map((day) => ({ label: day.label, seconds: day.seconds })),
    [data]
  );

  const start = async () => {
    setBusy(true);
    try {
      await api.startTimer(projectId || null, description);
      setDescription("");
      notify("הטיימר הופעל. עבודה נעימה!");
    } catch (error) {
      notify(errorText(error), "error");
    } finally {
      setBusy(false);
    }
  };

  const stop = async () => {
    setBusy(true);
    try {
      await api.stopTimer();
      notify("הרשומה נשמרה ביומן העבודה");
    } catch (error) {
      notify(errorText(error), "error");
    } finally {
      setBusy(false);
    }
  };

  const startQuick = async (id: string) => {
    try {
      await api.startTimer(id);
      notify("הטיימר הופעל");
    } catch (error) {
      notify(errorText(error), "error");
    }
  };

  return (
    <div className="page">
      <div className="grid grid-4">
        <Stat
          label="שעות היום"
          value={fmtHM(todaySeconds)}
          sub={`יעד יומי ${goalHours} שעות`}
          tone="accent"
        />
        <Stat
          label="שעות השבוע"
          value={fmtHM(data?.weekSeconds ?? 0)}
          sub={`${data?.entriesToday ?? 0} רשומות היום`}
        />
        <Stat
          label="הכנסות היום"
          value={fmtMoney(data?.todayEarnings ?? 0, currency)}
          sub={`שבועי ${fmtMoney(data?.weekEarnings ?? 0, currency)}`}
          tone="ok"
        />
        <Stat
          label="ממתין לתשלום"
          value={fmtMoney(data?.unpaidTotal ?? 0, currency)}
          sub={`${data?.activeClients ?? 0} לקוחות פעילים`}
          tone={data && data.unpaidTotal > 0 ? "warn" : undefined}
        />
      </div>

      <div className="grid grid-timer">
        <Card
          className="card-hero"
          title="הטיימר שלי"
          subtitle={timer.running ? "העבודה נמדדת בזמן אמת" : "בחרו פרויקט והתחילו לעבוד"}
          actions={<Badge tone={timer.running ? "ok" : "neutral"}>{timer.running ? "פעיל" : "ממוכן"}</Badge>}
        >
          <div className="timer-display">
            <div className={`timer-clock ${timer.running ? "is-running" : ""}`}>
              {fmtClock(timer.elapsedSec)}
            </div>
            <div className="timer-meta">
              <strong>{timer.running ? timer.projectName ?? "ללא פרויקט" : "לא פעיל"}</strong>
              <span>
                {timer.running
                  ? `${timer.clientName ?? "ללא לקוח"} · ${fmtMoney(timer.rate, currency)} לשעה`
                  : "ברגע שמתחילים, הכול נרשם אוטומטית לפרויקט וללקוח"}
              </span>
              {timer.running && (
                <span className="mono small">שווי נוכחי: {fmtMoney(liveEarnings, currency)}</span>
              )}
            </div>
            <div className="spacer" />
            <Button
              variant={timer.running ? "danger" : "primary"}
              size="lg"
              icon={timer.running ? "stop" : "play"}
              loading={busy}
              onClick={() => void (timer.running ? stop() : start())}
            >
              {timer.running ? "עצור וסכם" : "התחל עבודה"}
            </Button>
          </div>

          {!timer.running && (
            <div className="timer-controls">
              <label className="field">
                <span className="field-label">פרויקט</span>
                <select
                  className="input select"
                  value={projectId}
                  onChange={(event) => setProjectId(event.target.value)}
                >
                  <option value="">ללא פרויקט</option>
                  {projects
                    .filter((project) => !project.archived)
                    .map((project) => (
                      <option key={project.id} value={project.id}>
                        {project.name}
                        {project.clientName ? ` · ${project.clientName}` : ""}
                      </option>
                    ))}
                </select>
              </label>
              <label className="field">
                <span className="field-label">מה עושים עכשיו?</span>
                <input
                  className="input"
                  placeholder="לדוגמה: עיצוב מסך הבית"
                  value={description}
                  maxLength={180}
                  onChange={(event) => setDescription(event.target.value)}
                  onKeyDown={(event) => event.key === "Enter" && void start()}
                />
              </label>
            </div>
          )}

          {projects.length > 0 ? (
            <div className="quick-projects">
              {projects
                .filter((project) => !project.archived)
                .slice(0, 6)
                .map((project) => (
                  <button
                    key={project.id}
                    type="button"
                    className={`chip ${timer.active?.projectId === project.id ? "is-active" : ""}`}
                    onClick={() => void startQuick(project.id)}
                    disabled={timer.running}
                  >
                    <span className="chip-dot" style={{ background: project.color }} />
                    {project.name}
                  </button>
                ))}
            </div>
          ) : (
            <div className="note" style={{ marginTop: 16 }}>
              <Icon name="info" size={16} />
              <span>
                עוד לא הוגדרו פרויקטים. אפשר להתחיל טיימר כללי, או להוסיף פרויקט ראשון כדי לשייך את
                השעות ללקוח.
              </span>
              <Button size="sm" variant="soft" onClick={() => navigate("projects")}>
                הוספת פרויקט
              </Button>
            </div>
          )}
        </Card>

        <Card title="היעד היומי" subtitle={`${workday} שעות ביום עבודה`}>
          <div className="row" style={{ justifyContent: "space-between", alignItems: "center" }}>
            <ProgressRing
              progress={goalSeconds > 0 ? todaySeconds / goalSeconds : 0}
              label={fmtHM(todaySeconds)}
              sub={goalSeconds > 0 ? `${Math.round((todaySeconds / goalSeconds) * 100)}% מהיעד` : "ללא יעד"}
            />
            <div className="stack" style={{ flex: 1 }}>
              <div className="kv">
                <dt>הכנסה היום</dt>
                <dd>{fmtMoney(data?.todayEarnings ?? 0, currency)}</dd>
                <dt>החודש</dt>
                <dd>{fmtHM(data?.monthSeconds ?? 0)}</dd>
                <dt>שולם עד היום</dt>
                <dd>{fmtMoney(data?.paidTotal ?? 0, currency)}</dd>
              </div>
              <div className="progress-bar">
                <span
                  style={{
                    width: `${Math.min(100, goalSeconds > 0 ? (todaySeconds / goalSeconds) * 100 : 0)}%`,
                  }}
                />
              </div>
              <span className="muted small">
                {goalSeconds > 0 && todaySeconds >= goalSeconds
                  ? "הגעתם ליעד היומי — כל הכבוד!"
                  : `נותרו ${fmtHM(Math.max(0, goalSeconds - todaySeconds))} עד ליעד`}
              </span>
            </div>
          </div>
        </Card>
      </div>

      <div className="grid grid-2">
        <Card title="השבוע במבט אחד" subtitle="שעות עבודה לפי יום">
          {weekData.length > 0 ? (
            <BarChart data={weekData} format={fmtHM} goalSeconds={goalSeconds} />
          ) : (
            <Empty title="אין נתונים לשבוע הזה" hint="התחילו טיימר והנתונים יופיעו כאן." />
          )}
        </Card>

        <Card
          title="רשומות אחרונות"
          actions={
            <Button size="sm" variant="ghost" onClick={() => navigate("entries")}>
              ליומן המלא
            </Button>
          }
          bodyClass="flush"
        >
          {recent.length === 0 ? (
            <Empty
              icon="clock"
              title="עוד אין רשומות"
              hint="לחצו על התחל עבודה בלוח הבקרה והרשומה הראשונה תיווצר אוטומטית."
            />
          ) : (
            <div className="list">
              {recent.map((entry) => (
                <div className="list-row" key={entry.id}>
                  <div className="list-main">
                    <strong>{entry.projectName ?? "ללא פרויקט"}</strong>
                    <span>
                      {relativeDay(entry.startedAt)} · {fmtTime(entry.startedAt)}
                      {entry.endedAt ? `–${fmtTime(entry.endedAt)}` : " · פעיל"}
                      {entry.description ? ` · ${entry.description}` : ""}
                    </span>
                  </div>
                  <div className="list-meta">
                    <strong>{fmtHM(entry.durationSec)}</strong>
                    <span>{fmtMoney(entry.amount, currency)}</span>
                  </div>
                </div>
              ))}
            </div>
          )}
        </Card>
      </div>
    </div>
  );
}
