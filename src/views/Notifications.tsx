import { useEffect, useState } from "react";
import { api, errorText } from "../lib/api";
import { settingBoolean, settingNumber, settingString, useApp } from "../lib/store";
import { Badge, Button, Card, Empty, Field, Input, Toggle } from "../components/ui";
import { Icon, type IconName } from "../components/Icon";
import { fmtDateTime } from "../lib/format";

const ICONS: Record<string, IconName> = {
  reminder: "clock",
  goal: "spark",
  update: "cloud",
  backup: "drive",
  warning: "bell",
  info: "info",
};

export default function Notifications() {
  const { settings, notify, reloadKey } = useApp();
  const [items, setItems] = useState<Awaited<ReturnType<typeof api.notifications>>>([]);
  const [idleMinutes, setIdleMinutes] = useState(settingNumber(settings, "idleReminderMinutes", 45));
  const [workStart, setWorkStart] = useState(settingString(settings, "workStart", "09:00"));
  const [workEnd, setWorkEnd] = useState(settingString(settings, "workEnd", "18:00"));

  const load = () => {
    api
      .notifications()
      .then(setItems)
      .catch((error) => notify(errorText(error), "error"));
  };

  useEffect(load, [reloadKey]);

  useEffect(() => {
    setIdleMinutes(settingNumber(settings, "idleReminderMinutes", 45));
    setWorkStart(settingString(settings, "workStart", "09:00"));
    setWorkEnd(settingString(settings, "workEnd", "18:00"));
  }, [settings]);

  const update = async (key: string, value: string) => {
    try {
      await api.saveSettings({ [key]: value });
      notify("ההגדרה נשמרה");
    } catch (error) {
      notify(errorText(error), "error");
    }
  };

  const unread = items.filter((item) => !item.read).length;

  return (
    <div className="page">
      <div className="page-head">
        <div>
          <h1 className="page-title">התראות</h1>
          <p className="page-sub">
            {unread > 0 ? `${unread} התראות שלא נקראו` : "הכול נקרא"} · תזכורות עבודה והודעות מערכת
          </p>
        </div>
        <div className="page-actions">
          <Button
            variant="ghost"
            icon="check"
            onClick={async () => {
              try {
                await Promise.all(items.filter((item) => !item.read).map((item) => api.markNotification(item.id, true)));
                load();
              } catch (error) {
                notify(errorText(error), "error");
              }
            }}
            disabled={unread === 0}
          >
            סימון הכול כנקרא
          </Button>
          <Button
            variant="ghost"
            icon="trash"
            onClick={async () => {
              if (!window.confirm("לנקות את כל ההיסטוריה של ההתראות?")) return;
              try {
                await api.clearNotifications();
                load();
              } catch (error) {
                notify(errorText(error), "error");
              }
            }}
            disabled={items.length === 0}
          >
            ניקוי
          </Button>
        </div>
      </div>

      <div className="grid grid-2">
        <Card title="מרכז ההתראות" bodyClass="flush">
          {items.length === 0 ? (
            <Empty
              icon="bell"
              title="אין התראות"
              hint="כאן יופיעו תזכורות להתחלת עבודה, עדכוני גרסה, גיבויים והודעות מערכת."
            />
          ) : (
            <div className="list">
              {items.map((item) => (
                <button
                  key={item.id}
                  type="button"
                  className="list-row"
                  style={{ background: "transparent", border: "none", textAlign: "start", width: "100%", cursor: "pointer" }}
                  onClick={async () => {
                    if (item.read) return;
                    try {
                      await api.markNotification(item.id, true);
                      load();
                    } catch (error) {
                      notify(errorText(error), "error");
                    }
                  }}
                >
                  <span
                    className="empty-icon"
                    style={{ width: 34, height: 34, borderRadius: 11, flex: "none" }}
                  >
                    <Icon name={ICONS[item.kind] ?? "info"} size={16} />
                  </span>
                  <div className="list-main">
                    <strong>{item.title}</strong>
                    <span style={{ whiteSpace: "normal" }}>{item.body}</span>
                  </div>
                  <div className="list-meta">
                    <span>{fmtDateTime(item.createdAt)}</span>
                    {!item.read && <Badge tone="accent">חדש</Badge>}
                  </div>
                </button>
              ))}
            </div>
          )}
        </Card>

        <Card title="תזכורות עבודה" subtitle="מתי ואיך התוכנה תזכיר לכם לעבוד">
          <div className="stack">
            <Toggle
              checked={settingBoolean(settings, "notificationsEnabled", true)}
              onChange={(value) => void update("notificationsEnabled", String(value))}
              label="התראות מופעלות"
              hint="התראות מערכת של Windows ומרכז ההתראות"
            />
            <div className="form-grid">
              <Field label="תזכורת כשהטיימר כבוי" hint="בדקות של חוסר פעילות (0 = כבוי)">
                <Input
                  type="number"
                  min={0}
                  step={5}
                  value={idleMinutes}
                  onChange={(event) => setIdleMinutes(Number(event.target.value))}
                  onBlur={() => void update("idleReminderMinutes", String(idleMinutes))}
                />
              </Field>
              <Field label="יעד יומי" hint="שעות לחיוב ליום">
                <Input
                  type="number"
                  min={0}
                  step="0.5"
                  value={settingNumber(settings, "dailyGoalHours", 8)}
                  onChange={(event) => void update("dailyGoalHours", event.target.value)}
                />
              </Field>
              <Field label="תחילת יום עבודה">
                <Input
                  type="time"
                  value={workStart}
                  onChange={(event) => setWorkStart(event.target.value)}
                  onBlur={() => void update("workStart", workStart)}
                />
              </Field>
              <Field label="סיום יום עבודה">
                <Input
                  type="time"
                  value={workEnd}
                  onChange={(event) => setWorkEnd(event.target.value)}
                  onBlur={() => void update("workEnd", workEnd)}
                />
              </Field>
            </div>
            <div className="note">
              <Icon name="info" size={16} />
              <span>
                התזכורות נשלחות רק בתוך שעות העבודה שהוגדרו, ובקצב מוגבל כדי לא להציק. תזכורת יעד
                נשלחת פעם ביום.
              </span>
            </div>
          </div>
        </Card>
      </div>
    </div>
  );
}
