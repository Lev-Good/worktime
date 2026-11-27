import { useState } from "react";
import { api, errorText } from "../lib/api";
import { useApp } from "../lib/store";
import type { ProfileInput } from "../lib/types";
import { Badge, Button, Card, Empty, Field, IconButton, Input, Modal, Select } from "../components/ui";
import { CURRENCY_OPTIONS, fmtHM, fmtMoney } from "../lib/format";

const COLORS = ["#6d8dff", "#5ee0b0", "#ffc46b", "#ff758f", "#b58cff"];

export default function Profiles() {
  const { profiles, currency, notify, refreshAll } = useApp();
  const [draft, setDraft] = useState<ProfileInput | null>(null);
  const [saving, setSaving] = useState(false);

  const save = async () => {
    if (!draft) return;
    if (!draft.name.trim()) {
      notify("יש להזין שם פרופיל", "error");
      return;
    }
    setSaving(true);
    try {
      await api.saveProfile({ ...draft, name: draft.name.trim() });
      notify("הפרופיל נשמר");
      setDraft(null);
      await refreshAll();
    } catch (error) {
      notify(errorText(error), "error");
    } finally {
      setSaving(false);
    }
  };

  const remove = async (id: string, name: string) => {
    if (
      !window.confirm(
        `למחוק את הפרופיל "${name}"? כל הנתונים שלו — לקוחות, פרויקטים, שעות ותשלומים — יימחקו.`
      )
    )
      return;
    try {
      await api.deleteProfile(id);
      notify("הפרופיל נמחק");
      await refreshAll();
    } catch (error) {
      notify(errorText(error), "error");
    }
  };

  return (
    <div className="page">
      <div className="page-head">
        <div>
          <h1 className="page-title">פרופילים</h1>
          <p className="page-sub">כל פרופיל מנהל לקוחות, פרויקטים ותעריפים בנפרד — למשל עבודה ופרילנס</p>
        </div>
        <div className="page-actions">
          <Button
            variant="primary"
            icon="plus"
            onClick={() =>
              setDraft({ id: null, name: "", hourlyRate: 0, currency, color: COLORS[0] })
            }
          >
            פרופיל חדש
          </Button>
        </div>
      </div>

      {profiles.length === 0 ? (
        <Card>
          <Empty title="אין פרופילים" hint="צרו פרופיל ראשון כדי להתחיל." />
        </Card>
      ) : (
        <div className="grid grid-3">
          {profiles.map((profile) => (
            <Card key={profile.id} className={profile.isActive ? "card-hero" : ""}>
              <div className="row" style={{ alignItems: "flex-start" }}>
                <span className="avatar" style={{ background: profile.color }}>
                  {profile.name.slice(0, 1)}
                </span>
                <div className="list-main">
                  <strong>{profile.name}</strong>
                  <span>{fmtMoney(profile.hourlyRate, profile.currency)} לשעה</span>
                </div>
                {profile.isActive ? <Badge tone="ok">פעיל</Badge> : <Badge tone="neutral">לא פעיל</Badge>}
              </div>

              <div className="kv" style={{ marginTop: 14 }}>
                <dt>סה״כ שעות</dt>
                <dd>{fmtHM(profile.totalSeconds)}</dd>
                <dt>מטבע</dt>
                <dd>{profile.currency}</dd>
              </div>

              <div className="row" style={{ marginTop: 16, justifyContent: "space-between" }}>
                <Button
                  size="sm"
                  variant={profile.isActive ? "outline" : "soft"}
                  disabled={profile.isActive}
                  onClick={async () => {
                    try {
                      await api.switchProfile(profile.id);
                      await refreshAll();
                      notify(`עברתם לפרופיל ${profile.name}`);
                    } catch (error) {
                      notify(errorText(error), "error");
                    }
                  }}
                >
                  {profile.isActive ? "הפרופיל הנוכחי" : "מעבר לפרופיל"}
                </Button>
                <div className="row">
                  <IconButton
                    icon="edit"
                    label="עריכה"
                    onClick={() =>
                      setDraft({
                        id: profile.id,
                        name: profile.name,
                        hourlyRate: profile.hourlyRate,
                        currency: profile.currency,
                        color: profile.color,
                      })
                    }
                  />
                  <IconButton
                    icon="trash"
                    label="מחיקה"
                    onClick={() => void remove(profile.id, profile.name)}
                  />
                </div>
              </div>
            </Card>
          ))}
        </div>
      )}

      <div className="note">
        <span>
          טיפ: הפרופיל מגדיר תעריף ברירת מחדל ומטבע. תעריף לקוח או פרויקט גוברים עליו, והתעריף
          נשמר לכל רשומת זמן ברגע יצירתה.
        </span>
      </div>

      <Modal
        open={draft !== null}
        title={draft?.id ? "עריכת פרופיל" : "פרופיל חדש"}
        subtitle="אפשר להחזיק מספר פרופילים במקביל ולעבור ביניהם מהתפריט בשורת המשימות."
        onClose={() => setDraft(null)}
        footer={
          <>
            <Button variant="primary" loading={saving} onClick={() => void save()}>
              שמירה
            </Button>
            <Button variant="ghost" onClick={() => setDraft(null)}>
              ביטול
            </Button>
          </>
        }
      >
        {draft && (
          <div className="form-grid">
            <Field label="שם הפרופיל" className="span-2">
              <Input
                autoFocus
                value={draft.name}
                onChange={(event) => setDraft({ ...draft, name: event.target.value })}
                placeholder="לדוגמה: פרילנס"
              />
            </Field>
            <Field label="תעריף שעתי ברירת מחדל">
              <Input
                type="number"
                min={0}
                step="0.5"
                value={draft.hourlyRate}
                onChange={(event) => setDraft({ ...draft, hourlyRate: Number(event.target.value) })}
              />
            </Field>
            <Field label="מטבע">
              <Select
                value={draft.currency ?? currency}
                options={CURRENCY_OPTIONS}
                onChange={(event) => setDraft({ ...draft, currency: event.target.value })}
              />
            </Field>
            <Field label="צבע" className="span-2">
              <div className="row">
                {COLORS.map((color) => (
                  <button
                    key={color}
                    type="button"
                    aria-label={`צבע ${color}`}
                    onClick={() => setDraft({ ...draft, color })}
                    style={{
                      width: 26,
                      height: 26,
                      borderRadius: 9,
                      background: color,
                      border:
                        draft.color === color ? "2px solid var(--text)" : "1px solid var(--border)",
                      cursor: "pointer",
                    }}
                  />
                ))}
              </div>
            </Field>
          </div>
        )}
      </Modal>
    </div>
  );
}
