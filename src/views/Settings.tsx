import { useEffect, useState } from "react";
import { api, errorText } from "../lib/api";
import { settingBoolean, settingNumber, settingString, useApp } from "../lib/store";
import type { DriveStatus, Settings, UpdateInfo } from "../lib/types";
import { Badge, Button, Card, Field, Input, Select, Toggle } from "../components/ui";
import { Icon } from "../components/Icon";
import { CURRENCY_OPTIONS, clampNumber, fmtDateTime } from "../lib/format";

function NumberSetting({
  label,
  hint,
  value,
  onSave,
  min = 0,
  max,
  step = 1,
}: {
  label: string;
  hint?: string;
  value: number;
  onSave: (value: number) => void;
  min?: number;
  max?: number;
  step?: number;
}) {
  const [text, setText] = useState(String(value));
  useEffect(() => setText(String(value)), [value]);
  return (
    <Field label={label} hint={hint}>
      <Input
        type="number"
        min={min}
        max={max}
        step={step}
        value={text}
        onChange={(event) => setText(event.target.value)}
        onBlur={() => onSave(clampNumber(text, value))}
        onKeyDown={(event) => event.key === "Enter" && onSave(clampNumber(text, value))}
      />
    </Field>
  );
}

export default function SettingsView() {
  const { settings, currency, version, notify, refreshAll, reloadKey } = useApp();
  const [local, setLocal] = useState<Settings>(settings);
  const [drive, setDrive] = useState<DriveStatus | null>(null);
  const [driveBusy, setDriveBusy] = useState(false);
  const [clientId, setClientId] = useState(settingString(settings, "gdriveClientId", ""));
  const [clientSecret, setClientSecret] = useState("");
  const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null);
  const [checking, setChecking] = useState(false);
  const [repo, setRepo] = useState(settingString(settings, "updateRepo", "Lev-Good/worktime"));

  useEffect(() => {
    setLocal(settings);
    setClientId(settingString(settings, "gdriveClientId", ""));
    setRepo(settingString(settings, "updateRepo", "Lev-Good/worktime"));
  }, [settings]);

  useEffect(() => {
    let alive = true;
    api
      .driveStatus()
      .then((status) => alive && setDrive(status))
      .catch(() => undefined);
    return () => {
      alive = false;
    };
  }, [reloadKey]);

  const save = async (key: string, value: string) => {
    setLocal((current) => ({ ...current, [key]: value }));
    try {
      const next = await api.saveSettings({ [key]: value });
      setLocal(next);
    } catch (error) {
      notify(errorText(error), "error");
    }
  };

  const driveStatus = drive;

  const connectDrive = async () => {
    if (!clientId.trim() || !clientSecret.trim()) {
      notify("יש למלא Client ID ו-Client Secret של אפליקציית Google", "error");
      return;
    }
    setDriveBusy(true);
    try {
      const status = await api.driveConnect(clientId, clientSecret);
      setDrive(status);
      setClientSecret("");
      notify("החיבור ל-Google Drive הושלם");
    } catch (error) {
      notify(errorText(error), "error");
    } finally {
      setDriveBusy(false);
    }
  };

  const backupNow = async () => {
    setDriveBusy(true);
    try {
      const status = await api.driveBackup();
      setDrive(status);
      notify("הגיבוי הועלה ל-Google Drive");
    } catch (error) {
      notify(errorText(error), "error");
    } finally {
      setDriveBusy(false);
    }
  };

  const restore = async (id: string, name: string) => {
    if (
      !window.confirm(
        `לשחזר את הגיבוי "${name}"? כל הנתונים הנוכחיים יוחלפו בתוכן הגיבוי. מומלץ לבצע גיבוי מקומי לפני כן.`
      )
    )
      return;
    setDriveBusy(true);
    try {
      await api.driveRestore(id);
      await refreshAll();
      notify("הנתונים שוחזרו בהצלחה");
    } catch (error) {
      notify(errorText(error), "error");
    } finally {
      setDriveBusy(false);
    }
  };

  const localBackup = async () => {
    try {
      const path = await api.backupLocal();
      notify(`גיבוי מקומי נשמר: ${path}`);
    } catch (error) {
      notify(errorText(error), "error");
    }
  };

  const checkUpdates = async () => {
    setChecking(true);
    try {
      const info = await api.checkUpdates(true);
      setUpdateInfo(info);
      notify(info.hasUpdate ? `גרסה ${info.latestVersion} זמינה להורדה` : "אתם מעודכנים בגרסה האחרונה");
    } catch (error) {
      notify(errorText(error), "error");
    } finally {
      setChecking(false);
    }
  };

  const opacity = settingNumber(local, "floatingOpacity", 0.94);

  return (
    <div className="page">
      <div className="page-head">
        <div>
          <h1 className="page-title">הגדרות</h1>
          <p className="page-sub">כל מה שקובע איך WorkTime מתנהג, נראה ומגבה את עצמו</p>
        </div>
      </div>

      <div className="grid grid-2">
        <Card title="מראה ושפה" subtitle="ערכת נושא ומטבח ברירת מחדל">
          <div className="form-grid">
            <Field label="ערכת נושא">
              <Select
                value={settingString(local, "theme", "dark")}
                options={[
                  { value: "dark", label: "כהה" },
                  { value: "light", label: "בהיר" },
                  { value: "system", label: "לפי Windows" },
                ]}
                onChange={(event) => void save("theme", event.target.value)}
              />
            </Field>
            <Field label="מטבע">
              <Select
                value={currency}
                options={CURRENCY_OPTIONS}
                onChange={(event) => void save("currency", event.target.value)}
              />
            </Field>
          </div>
        </Card>

        <Card title="עבודה ותמחור" subtitle="ברירות מחדל לחישוב שעות וחיובים">
          <div className="form-grid">
            <NumberSetting
              label="יום עבודה (שעות)"
              value={settingNumber(local, "workdayHours", 8.5)}
              step={0.5}
              onSave={(value) => void save("workdayHours", String(value))}
            />
            <NumberSetting
              label="יעד יומי (שעות)"
              value={settingNumber(local, "dailyGoalHours", 8)}
              step={0.5}
              onSave={(value) => void save("dailyGoalHours", String(value))}
            />
            <Field label="עיגול דקות" hint="מעגל את משך הרשומה כלפי מעלה">
              <Select
                value={settingString(local, "roundingMinutes", "0")}
                options={[
                  { value: "0", label: "ללא עיגול" },
                  { value: "5", label: "5 דקות" },
                  { value: "10", label: "10 דקות" },
                  { value: "15", label: "15 דקות" },
                  { value: "30", label: "30 דקות" },
                ]}
                onChange={(event) => void save("roundingMinutes", event.target.value)}
              />
            </Field>
            <NumberSetting
              label="סגירה אוטומטית (שעות)"
              hint="טיימר שנשכח ייסגר אחרי מספר השעות הזה"
              value={settingNumber(local, "autoStopHours", 12)}
              min={1}
              onSave={(value) => void save("autoStopHours", String(value))}
            />
          </div>
          <div style={{ marginTop: 8 }}>
            <Toggle
              checked={settingBoolean(local, "billableDefault", true)}
              onChange={(value) => void save("billableDefault", String(value))}
              label="רשומות חדשות מסומנות לחיוב"
              hint="ניתן לשנות לכל רשומה בנפרד"
            />
          </div>
        </Card>
      </div>

      <div className="grid grid-2">
        <Card title="שעון צף" subtitle="חלון מינימליסטי שקוף שתמיד על המסך">
          <Toggle
            checked={settingBoolean(local, "floatingEnabled", false)}
            onChange={async (value) => {
              try {
                await api.setFloating(value);
                setLocal((current) => ({ ...current, floatingEnabled: String(value) }));
              } catch (error) {
                notify(errorText(error), "error");
              }
            }}
            label="הצגת השעון הצף"
            hint="אפשר לגרור אותו לכל מקום על המסך"
          />
          <Toggle
            checked={settingBoolean(local, "floatingSeconds", true)}
            onChange={(value) => void save("floatingSeconds", String(value))}
            label="הצגת שניות"
          />
          <Toggle
            checked={settingBoolean(local, "floatingCompact", false)}
            onChange={(value) => void save("floatingCompact", String(value))}
            label="מצב קומפקטי"
            hint="רק השעון, בלי שם הפרויקט"
          />
          <Toggle
            checked={settingBoolean(local, "floatingAlwaysOnTop", true)}
            onChange={(value) => void save("floatingAlwaysOnTop", String(value))}
            label="תמיד מעל חלונות אחרים"
          />
          <Field label={`אטימות השעון — ${Math.round(opacity * 100)}%`}>
            <input
              type="range"
              min={0.6}
              max={1}
              step={0.02}
              value={opacity}
              onChange={(event) =>
                setLocal((current) => ({ ...current, floatingOpacity: event.target.value }))
              }
              onPointerUp={(event) => void save("floatingOpacity", event.currentTarget.value)}
            />
          </Field>
        </Card>

        <Card title="שורת המשימות והפעלה" subtitle="שליטה מלאה בלי לפתוח את החלון">
          <Toggle
            checked={settingBoolean(local, "trayShowTimer", true)}
            onChange={(value) => void save("trayShowTimer", String(value))}
            label="הצגת הטיימר בתפריט שורת המשימות"
            hint="הזמן והפרויקט הפעיל מופיעים בראש התפריט"
          />
          <Toggle
            checked={settingBoolean(local, "autostart", false)}
            onChange={(value) => void save("autostart", String(value))}
            label="הפעלה אוטומטית עם Windows"
            hint="התוכנה תעלה minimized לשורת המשימות"
          />
          <Toggle
            checked={settingBoolean(local, "notificationsEnabled", true)}
            onChange={(value) => void save("notificationsEnabled", String(value))}
            label="התראות מערכת"
          />
          <div className="note">
            <Icon name="info" size={16} />
            <span>
              סגירת החלון משאירה את התוכנה פועלת ברקע. ליציאה מלאה אפשר להשתמש ב־״יציאה״ בתפריט
              שורת המשימות.
            </span>
          </div>
        </Card>
      </div>

      <Card
        title="גיבוי ושחזור"
        subtitle="גיבוי מקומי או לחשבון Google Drive — קובץ JSON אחד עם כל הנתונים"
        actions={<Badge tone={driveStatus?.connected ? "ok" : "neutral"}>{driveStatus?.connected ? "מחובר" : "לא מחובר"}</Badge>}
      >
        <div className="stack" style={{ gap: 16 }}>
          <div className="row">
            <Button variant="soft" icon="download" onClick={() => void localBackup()}>
              גיבוי מקומי עכשיו
            </Button>
            <span className="muted small">נשמר בתיקיית WorkTime/backups שבמסמכים</span>
          </div>

          <div className="divider" />

          <div className="row" style={{ alignItems: "flex-start", gap: 16 }}>
            <div className="stack" style={{ flex: 2, minWidth: 280 }}>
              <div className="section-title">Google Drive</div>
              <p className="muted small">
                כדי לחבר את Drive צריך ליצור אפליקציית OAuth מסוג Desktop ב-Google Cloud Console,
                ולהדביק כאן את ה-Client ID וה-Client Secret. ההרשאה היחידה המבוקשת היא{" "}
                <span className="mono">drive.file</span> — גישה רק לקבצים שהתוכנה יצרה.
              </p>
              <div className="form-grid">
                <Field label="Client ID">
                  <Input
                    value={clientId}
                    onChange={(event) => setClientId(event.target.value)}
                    placeholder="xxxx.apps.googleusercontent.com"
                  />
                </Field>
                <Field label="Client Secret" hint="נשמר מקומית במחשב בלבד">
                  <Input
                    type="password"
                    value={clientSecret}
                    onChange={(event) => setClientSecret(event.target.value)}
                    placeholder="••••••••"
                  />
                </Field>
              </div>
              <div className="row">
                <Button
                  variant="primary"
                  icon="drive"
                  loading={driveBusy}
                  onClick={() => void connectDrive()}
                >
                  {driveStatus?.connected ? "חיבור מחדש" : "התחברות ל-Google Drive"}
                </Button>
                {driveStatus?.connected && (
                  <>
                    <Button variant="soft" icon="cloud" loading={driveBusy} onClick={() => void backupNow()}>
                      גיבוי עכשיו
                    </Button>
                    <Button
                      variant="ghost"
                      onClick={async () => {
                        try {
                          await api.driveDisconnect();
                          setDrive(await api.driveStatus());
                          notify("החיבור ל-Drive נותק");
                        } catch (error) {
                          notify(errorText(error), "error");
                        }
                      }}
                    >
                      ניתוק
                    </Button>
                  </>
                )}
              </div>
              {driveStatus?.connected && (
                <span className="muted small">
                  {driveStatus.account || "מחובר"} · גיבוי אחרון:{" "}
                  {driveStatus.lastBackupAt ? fmtDateTime(driveStatus.lastBackupAt) : "טרם בוצע"}
                </span>
              )}
            </div>

            <div className="stack" style={{ flex: 1, minWidth: 220 }}>
              <Toggle
                checked={settingBoolean(local, "backupEnabled", false)}
                onChange={(value) => void save("backupEnabled", String(value))}
                label="גיבוי אוטומטי"
                hint="רץ ברקע כל עוד התוכנה פתוחה"
              />
              <NumberSetting
                label="תדירות (שעות)"
                value={settingNumber(local, "backupIntervalHours", 24)}
                min={1}
                onSave={(value) => void save("backupIntervalHours", String(value))}
              />
            </div>
          </div>

          {driveStatus?.connected && (
            <div className="stack">
              <div className="section-title">גיבויים בדרייב ({driveStatus.files.length})</div>
              {driveStatus.files.length === 0 ? (
                <p className="muted small">עוד אין גיבויים בתיקיית WorkTime Backups.</p>
              ) : (
                <div className="table-wrap">
                  <table className="table">
                    <thead>
                      <tr>
                        <th>שם הקובץ</th>
                        <th>תאריך</th>
                        <th className="num">גודל</th>
                        <th />
                      </tr>
                    </thead>
                    <tbody>
                      {driveStatus.files.map((file) => (
                        <tr key={file.id}>
                          <td className="mono small">{file.name}</td>
                          <td>{fmtDateTime(file.createdAt.replace("T", " ").slice(0, 19))}</td>
                          <td className="num">{Math.max(1, Math.round(file.size / 1024))} KB</td>
                          <td>
                            <div className="row-actions">
                              <Button
                                size="sm"
                                variant="ghost"
                                icon="refresh"
                                onClick={() => void restore(file.id, file.name)}
                              >
                                שחזור
                              </Button>
                            </div>
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              )}
            </div>
          )}
        </div>
      </Card>

      <Card
        title="עדכוני גרסה"
        subtitle="בדיקה מול דף המהדורה ב-GitHub, כולל פירוט מה חדש"
        actions={<Badge tone="accent">גרסה {version}</Badge>}
      >
        <div className="grid grid-2" style={{ gap: 16 }}>
          <div className="stack">
            <Field label="מאגר העדכונים" hint="בפורמט owner/repo">
              <Input
                value={repo}
                onChange={(event) => setRepo(event.target.value)}
                onBlur={() => void save("updateRepo", repo.trim())}
              />
            </Field>
            <Toggle
              checked={settingBoolean(local, "autoCheckUpdates", true)}
              onChange={(value) => void save("autoCheckUpdates", String(value))}
              label="בדיקת עדכונים אוטומטית"
              hint="פעם ב-12 שעות, ובשקט ברקע"
            />
            <div className="row">
              <Button variant="primary" icon="cloud" loading={checking} onClick={() => void checkUpdates()}>
                בדיקת עדכון
              </Button>
              {updateInfo && (
                <Button variant="ghost" onClick={() => void api.openUrl(updateInfo.pageUrl)}>
                  דף המהדורה
                </Button>
              )}
            </div>
            <span className="muted small">
              הבדיקה משתמשת ב-GitHub Releases API ומציגה את הערות הגרסה ואת קובצי ההתקנה.
            </span>
          </div>

          <div className="stack">
            {updateInfo ? (
              <>
                <div className="row">
                  <strong>{updateInfo.title}</strong>
                  {updateInfo.hasUpdate ? (
                    <Badge tone="ok">גרסה {updateInfo.latestVersion} זמינה</Badge>
                  ) : (
                    <Badge tone="neutral">מעודכן</Badge>
                  )}
                </div>
                {updateInfo.publishedAt && (
                  <span className="muted small">
                    פורסם: {fmtDateTime(updateInfo.publishedAt.slice(0, 19).replace("T", " "))}
                  </span>
                )}
                <div className="release-notes">
                  {updateInfo.notes || "לא צורפו הערות גרסה למהדורה הזו."}
                </div>
                {updateInfo.assets.length > 0 && (
                  <div className="stack" style={{ gap: 6 }}>
                    <span className="section-title">קובצי התקנה</span>
                    {updateInfo.assets.map((asset) => (
                      <div className="row" key={asset.url}>
                        <Button
                          size="sm"
                          variant="soft"
                          icon="download"
                          onClick={() => void api.openUrl(asset.url)}
                        >
                          הורדה
                        </Button>
                        <span className="small mono">{asset.name}</span>
                        <span className="muted small">
                          {Math.max(1, Math.round(asset.size / 1024 / 1024))} MB
                        </span>
                      </div>
                    ))}
                  </div>
                )}
              </>
            ) : (
              <div className="note">
                <Icon name="info" size={16} />
                <span>
                  לחצו על בדיקת עדכון כדי לראות את הגרסה האחרונה שפורסמה, מה נוסף בה, וקישור
                  להורדה.
                </span>
              </div>
            )}
          </div>
        </div>
      </Card>

      <Card title="אודות">
        <div className="row" style={{ justifyContent: "space-between", alignItems: "flex-start" }}>
          <div className="stack" style={{ gap: 6 }}>
            <strong>WorkTime {version}</strong>
            <span className="muted small">
              ניהול זמן עבודה קומפקטי ל-Windows · Tauri + Rust + SQLite · קובץ התקנה קטן וצריכת
              זיכרון נמוכה.
            </span>
            <span className="muted small">
              הנתונים נשמרים מקומית במחשב. גיבוי ל-Google Drive מתבצע רק ביוזמתכם.
            </span>
          </div>
        </div>
      </Card>
    </div>
  );
}
