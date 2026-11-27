import { useEffect, useMemo, useState } from "react";
import { api, errorText } from "../lib/api";
import { useApp } from "../lib/store";
import type { Project, ProjectInput } from "../lib/types";
import { Badge, Button, Card, Empty, Field, IconButton, Input, Modal, Select, Textarea } from "../components/ui";
import { fmtHM, fmtMoney, relativeDay } from "../lib/format";

const COLORS = ["#6d8dff", "#5ee0b0", "#ffc46b", "#ff758f", "#b58cff", "#4ec9d9"];

const EMPTY: ProjectInput = {
  id: null,
  name: "",
  clientId: null,
  description: "",
  status: "active",
  hourlyRate: null,
  budget: null,
  color: COLORS[0],
  archived: false,
};

const STATUS_LABEL: Record<string, string> = {
  active: "פעיל",
  paused: "בהמתנה",
  done: "הושלם",
};

export default function Projects() {
  const { currency, clients, notify, reloadKey } = useApp();
  const [items, setItems] = useState<Project[]>([]);
  const [filter, setFilter] = useState<"active" | "all" | "archived">("active");
  const [draft, setDraft] = useState<ProjectInput | null>(null);
  const [saving, setSaving] = useState(false);

  const load = () => {
    api
      .projects()
      .then(setItems)
      .catch((error) => notify(errorText(error), "error"));
  };

  useEffect(load, [reloadKey]);

  const filtered = useMemo(() => {
    if (filter === "all") return items;
    if (filter === "archived") return items.filter((project) => project.archived);
    return items.filter((project) => !project.archived);
  }, [items, filter]);

  const save = async () => {
    if (!draft) return;
    if (!draft.name.trim()) {
      notify("יש להזין שם פרויקט", "error");
      return;
    }
    setSaving(true);
    try {
      await api.saveProject({ ...draft, name: draft.name.trim() });
      notify("הפרויקט נשמר");
      setDraft(null);
      load();
    } catch (error) {
      notify(errorText(error), "error");
    } finally {
      setSaving(false);
    }
  };

  const remove = async (project: Project) => {
    if (!window.confirm(`למחוק את הפרויקט "${project.name}"? רשומות הזמן יישארו ביומן ללא שיוך.`))
      return;
    try {
      await api.deleteProject(project.id);
      notify("הפרויקט נמחק");
      load();
    } catch (error) {
      notify(errorText(error), "error");
    }
  };

  const clientOptions = clients.map((client) => ({ value: client.id, label: client.name }));

  return (
    <div className="page">
      <div className="page-head">
        <div>
          <h1 className="page-title">פרויקטים</h1>
          <p className="page-sub">
            {items.filter((project) => !project.archived).length} פרויקטים פעילים ·{" "}
            {fmtHM(items.reduce((total, project) => total + project.totalSeconds, 0))} שעות מתועדות
          </p>
        </div>
        <div className="page-actions">
          <div className="segmented">
            {(["active", "all", "archived"] as const).map((value) => (
              <button
                key={value}
                type="button"
                className={filter === value ? "is-active" : ""}
                onClick={() => setFilter(value)}
              >
                {value === "active" ? "פעילים" : value === "all" ? "הכול" : "ארכיון"}
              </button>
            ))}
          </div>
          <Button variant="primary" icon="plus" onClick={() => setDraft({ ...EMPTY })}>
            פרויקט חדש
          </Button>
        </div>
      </div>

      {filtered.length === 0 ? (
        <Card>
          <Empty
            icon="folder"
            title={filter === "archived" ? "אין פרויקטים בארכיון" : "עוד אין פרויקטים"}
            hint="פרויקט משייך שעות ללקוח, קובע תעריף ומאפשר מעקב תקציב."
            action={
              filter !== "archived" ? (
                <Button variant="primary" icon="plus" onClick={() => setDraft({ ...EMPTY })}>
                  הוספת פרויקט
                </Button>
              ) : undefined
            }
          />
        </Card>
      ) : (
        <div className="grid grid-3">
          {filtered.map((project) => {
            const rate = project.hourlyRate ?? 0;
            const value = rate * (project.totalSeconds / 3600);
            const budgetProgress =
              project.budget && project.budget > 0 ? Math.min(1, value / project.budget) : null;
            return (
              <Card key={project.id} className="card-hero">
                <div className="row" style={{ alignItems: "flex-start" }}>
                  <span className="avatar" style={{ background: project.color }}>
                    <span style={{ fontSize: 12 }}>{project.name.slice(0, 2)}</span>
                  </span>
                  <div className="list-main">
                    <strong>{project.name}</strong>
                    <span>{project.clientName ?? "ללא לקוח"}</span>
                  </div>
                  <Badge tone={project.archived ? "neutral" : project.status === "done" ? "ok" : "accent"}>
                    {project.archived ? "ארכיון" : STATUS_LABEL[project.status] ?? project.status}
                  </Badge>
                </div>

                {project.description && (
                  <p className="muted small" style={{ marginTop: 10 }}>
                    {project.description}
                  </p>
                )}

                <div className="kv" style={{ marginTop: 12 }}>
                  <dt>שעות</dt>
                  <dd>{fmtHM(project.totalSeconds)}</dd>
                  <dt>שווי עבודה</dt>
                  <dd>{fmtMoney(value, currency)}</dd>
                  <dt>ממתין לתשלום</dt>
                  <dd>{fmtMoney(project.unpaidAmount, currency)}</dd>
                  <dt>עדכון אחרון</dt>
                  <dd>{project.lastUsedAt ? relativeDay(project.lastUsedAt) : "—"}</dd>
                </div>

                {budgetProgress !== null && (
                  <div className="stack" style={{ marginTop: 12, gap: 6 }}>
                    <div className="progress-bar">
                      <span style={{ width: `${budgetProgress * 100}%` }} />
                    </div>
                    <span className="small muted">
                      {Math.round(budgetProgress * 100)}% מתקציב של{" "}
                      {fmtMoney(project.budget ?? 0, currency)}
                    </span>
                  </div>
                )}

                <div className="row" style={{ marginTop: 14, justifyContent: "flex-end" }}>
                  <IconButton
                    icon="edit"
                    label="עריכה"
                    onClick={() =>
                      setDraft({
                        id: project.id,
                        name: project.name,
                        clientId: project.clientId,
                        description: project.description,
                        status: project.status,
                        hourlyRate: project.hourlyRate,
                        budget: project.budget,
                        color: project.color,
                        archived: project.archived,
                      })
                    }
                  />
                  <IconButton icon="trash" label="מחיקה" onClick={() => void remove(project)} />
                </div>
              </Card>
            );
          })}
        </div>
      )}

      <Modal
        open={draft !== null}
        title={draft?.id ? "עריכת פרויקט" : "פרויקט חדש"}
        subtitle="תעריף הפרויקט גובר על תעריף הלקוח, וזה גובר על תעריף הפרופיל."
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
            <Field label="שם הפרויקט" className="span-2">
              <Input
                autoFocus
                value={draft.name}
                onChange={(event) => setDraft({ ...draft, name: event.target.value })}
                placeholder="לדוגמה: אתר תדמית"
              />
            </Field>
            <Field label="לקוח">
              <Select
                value={draft.clientId ?? ""}
                options={clientOptions}
                placeholder="ללא לקוח"
                onChange={(event) =>
                  setDraft({ ...draft, clientId: event.target.value || null })
                }
              />
            </Field>
            <Field label="סטטוס">
              <Select
                value={draft.status}
                options={[
                  { value: "active", label: "פעיל" },
                  { value: "paused", label: "בהמתנה" },
                  { value: "done", label: "הושלם" },
                ]}
                onChange={(event) => setDraft({ ...draft, status: event.target.value })}
              />
            </Field>
            <Field label="תעריף שעתי" hint="ריק = לפי הלקוח או הפרופיל">
              <Input
                type="number"
                min={0}
                step="0.5"
                value={draft.hourlyRate ?? ""}
                onChange={(event) =>
                  setDraft({
                    ...draft,
                    hourlyRate: event.target.value === "" ? null : Number(event.target.value),
                  })
                }
              />
            </Field>
            <Field label="תקציב" hint="למעקב התקדמות מול התקציב">
              <Input
                type="number"
                min={0}
                step="10"
                value={draft.budget ?? ""}
                onChange={(event) =>
                  setDraft({
                    ...draft,
                    budget: event.target.value === "" ? null : Number(event.target.value),
                  })
                }
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
                        draft.color === color
                          ? "2px solid var(--text)"
                          : "1px solid var(--border)",
                      cursor: "pointer",
                    }}
                  />
                ))}
              </div>
            </Field>
            <Field label="תיאור" className="span-2">
              <Textarea
                value={draft.description}
                onChange={(event) => setDraft({ ...draft, description: event.target.value })}
                placeholder="מה כולל הפרויקט, אבני דרך, הערות…"
              />
            </Field>
            <Field label="ארכיון" hint="העברה לארכיון מסתירה את הפרויקט מהרשימה הפעילה">
              <Select
                value={draft.archived ? "yes" : "no"}
                options={[
                  { value: "no", label: "פעיל ברשימה" },
                  { value: "yes", label: "בארכיון" },
                ]}
                onChange={(event) => setDraft({ ...draft, archived: event.target.value === "yes" })}
              />
            </Field>
          </div>
        )}
      </Modal>
    </div>
  );
}
