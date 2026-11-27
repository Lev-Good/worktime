import { useEffect, useMemo, useState } from "react";
import { api, errorText } from "../lib/api";
import { useApp } from "../lib/store";
import type { Entry, EntryInput } from "../lib/types";
import { Badge, Button, Card, Empty, Field, IconButton, Input, Modal, Select, Textarea } from "../components/ui";
import { Icon } from "../components/Icon";
import {
  fmtDate,
  fmtHM,
  fmtMoney,
  fmtTime,
  normalizeDateTime,
  nowInputDateTime,
  shiftDays,
  startOfMonthISO,
  startOfWeekISO,
  todayISO,
  toInputDateTime,
} from "../lib/format";

type Preset = "today" | "week" | "month" | "custom";

/** Rows fetched per request; the list appends another page on demand. */
const PAGE_SIZE = 100;

export default function Entries() {
  const { currency, projects, clients, notify, reloadKey, timer } = useApp();
  const [entries, setEntries] = useState<Entry[]>([]);
  const [preset, setPreset] = useState<Preset>("week");
  const [from, setFrom] = useState(startOfWeekISO());
  const [to, setTo] = useState(todayISO());
  const [clientId, setClientId] = useState("");
  const [projectId, setProjectId] = useState("");
  const [search, setSearch] = useState("");
  const [query, setQuery] = useState("");
  const [summary, setSummary] = useState({ total: 0, seconds: 0, amount: 0 });
  const [offset, setOffset] = useState(0);
  const [hasMore, setHasMore] = useState(false);
  const [loadingMore, setLoadingMore] = useState(false);
  const [draft, setDraft] = useState<EntryInput | null>(null);
  const [saving, setSaving] = useState(false);

  const range = useMemo(() => {
    if (preset === "today") return { from: todayISO(), to: todayISO() };
    if (preset === "week") return { from: startOfWeekISO(), to: todayISO() };
    if (preset === "month") return { from: startOfMonthISO(), to: todayISO() };
    return { from, to };
  }, [preset, from, to]);

  // Typing in the search box waits for a pause instead of querying on every keystroke.
  useEffect(() => {
    const pending = window.setTimeout(() => setQuery(search.trim()), 300);
    return () => window.clearTimeout(pending);
  }, [search]);

  const fetchPage = (pageOffset: number) =>
    api.entries({
      from: range.from,
      to: range.to,
      clientId: clientId || null,
      projectId: projectId || null,
      search: query || null,
      limit: PAGE_SIZE,
      offset: pageOffset,
    });

  const visible = (rows: Entry[]) =>
    rows.filter((entry) => entry.endedAt !== null || entry.id === timer.active?.id);

  const load = () => {
    fetchPage(0)
      .then((data) => {
        setEntries(visible(data.entries));
        setSummary({ total: data.total, seconds: data.totalSeconds, amount: data.totalAmount });
        setOffset(data.entries.length);
        setHasMore(data.hasMore);
      })
      .catch((error) => notify(errorText(error), "error"));
  };

  const loadMore = async () => {
    setLoadingMore(true);
    try {
      const data = await fetchPage(offset);
      setEntries((current) => [...current, ...visible(data.entries)]);
      setOffset(offset + data.entries.length);
      setHasMore(data.hasMore);
    } catch (error) {
      notify(errorText(error), "error");
    } finally {
      setLoadingMore(false);
    }
  };

  useEffect(load, [
    reloadKey,
    range.from,
    range.to,
    clientId,
    projectId,
    query,
    timer.active?.id,
  ]);

  const save = async () => {
    if (!draft) return;
    setSaving(true);
    try {
      await api.saveEntry({
        ...draft,
        startedAt: normalizeDateTime(draft.startedAt),
        endedAt: normalizeDateTime(draft.endedAt),
      });
      notify("הרשומה נשמרה");
      setDraft(null);
      load();
    } catch (error) {
      notify(errorText(error), "error");
    } finally {
      setSaving(false);
    }
  };

  const remove = async (entry: Entry) => {
    if (!window.confirm("למחוק את הרשומה? הפעולה אינה ניתנת לביטוי.")) return;
    try {
      await api.deleteEntry(entry.id);
      notify("הרשומה נמחקה");
      load();
    } catch (error) {
      notify(errorText(error), "error");
    }
  };

  const togglePaid = async (entry: Entry) => {
    try {
      await api.setEntryPaid(entry.id, !entry.paid);
      load();
    } catch (error) {
      notify(errorText(error), "error");
    }
  };

  const openNew = () =>
    setDraft({
      id: null,
      projectId: projectId || projects[0]?.id || null,
      description: "",
      startedAt: nowInputDateTime(),
      endedAt: nowInputDateTime(),
      billable: true,
      paid: false,
    });

  const openEdit = (entry: Entry) =>
    setDraft({
      id: entry.id,
      projectId: entry.projectId,
      description: entry.description,
      startedAt: toInputDateTime(entry.startedAt),
      endedAt: toInputDateTime(entry.endedAt ?? entry.startedAt),
      billable: entry.billable,
      paid: entry.paid,
    });

  return (
    <div className="page">
      <div className="page-head">
        <div>
          <h1 className="page-title">יומן עבודה</h1>
          <p className="page-sub">
            {summary.total} רשומות · {fmtHM(summary.seconds)} שעות ·{" "}
            {fmtMoney(summary.amount, currency)}
            {entries.length < summary.total ? ` · מוצגות ${entries.length}` : ""}
          </p>
        </div>
        <div className="page-actions">
          <Button variant="primary" icon="plus" onClick={openNew}>
            רשומה ידנית
          </Button>
        </div>
      </div>

      <Card>
        <div className="filters">
          <div className="segmented" style={{ alignSelf: "flex-end" }}>
            {(
              [
                { value: "today", label: "היום" },
                { value: "week", label: "השבוע" },
                { value: "month", label: "החודש" },
                { value: "custom", label: "טווח" },
              ] as { value: Preset; label: string }[]
            ).map((option) => (
              <button
                key={option.value}
                type="button"
                className={preset === option.value ? "is-active" : ""}
                onClick={() => setPreset(option.value)}
              >
                {option.label}
              </button>
            ))}
          </div>

          {preset === "custom" && (
            <>
              <Field label="מתאריך">
                <Input type="date" value={from} onChange={(event) => setFrom(event.target.value)} />
              </Field>
              <Field label="עד תאריך">
                <Input
                  type="date"
                  value={to}
                  min={from}
                  max={shiftDays(todayISO(), 1)}
                  onChange={(event) => setTo(event.target.value)}
                />
              </Field>
            </>
          )}

          <Field label="לקוח">
            <Select
              value={clientId}
              placeholder="כל הלקוחות"
              options={clients.map((client) => ({ value: client.id, label: client.name }))}
              onChange={(event) => setClientId(event.target.value)}
            />
          </Field>
          <Field label="פרויקט">
            <Select
              value={projectId}
              placeholder="כל הפרויקטים"
              options={projects.map((project) => ({ value: project.id, label: project.name }))}
              onChange={(event) => setProjectId(event.target.value)}
            />
          </Field>
          <Field label="חיפוש">
            <Input
              value={search}
              placeholder="חיפוש בתיאור…"
              onChange={(event) => setSearch(event.target.value)}
            />
          </Field>
          <div className="spacer" />
          <Button variant="ghost" icon="refresh" onClick={load}>
            רענון
          </Button>
        </div>
      </Card>

      <Card bodyClass="flush">
        {entries.length === 0 ? (
          <Empty
            icon="clock"
            title="אין רשומות בטווח הזה"
            hint="אפשר להתחיל טיימר מלוח הבקרה, או להוסיף רשומה ידנית עבור עבודה שכבר בוצעה."
            action={
              <Button variant="soft" icon="plus" onClick={openNew}>
                הוספת רשומה ידנית
              </Button>
            }
          />
        ) : (
          <div className="table-wrap">
            <table className="table">
              <thead>
                <tr>
                  <th>תאריך</th>
                  <th>שעות</th>
                  <th>פרויקט</th>
                  <th>תיאור</th>
                  <th className="num">משך</th>
                  <th className="num">סכום</th>
                  <th>סטטוס</th>
                  <th />
                </tr>
              </thead>
              <tbody>
                {entries.map((entry) => (
                  <tr key={entry.id}>
                    <td className="nowrap">{fmtDate(entry.startedAt)}</td>
                    <td className="mono">
                      {fmtTime(entry.startedAt)}–{entry.endedAt ? fmtTime(entry.endedAt) : "פעיל"}
                    </td>
                    <td>
                      <div className="cell-title">
                        <span className="strong">{entry.projectName ?? "ללא פרויקט"}</span>
                        <small>{entry.clientName ?? "ללא לקוח"}</small>
                      </div>
                    </td>
                    <td style={{ maxWidth: 260 }}>
                      <span className="muted small">{entry.description || "—"}</span>
                    </td>
                    <td className="num strong">{fmtHM(entry.durationSec)}</td>
                    <td className="num">{fmtMoney(entry.amount, currency)}</td>
                    <td>
                      <button
                        type="button"
                        onClick={() => void togglePaid(entry)}
                        title="שינוי סטטוס תשלום"
                        style={{ background: "none", border: "none", padding: 0, cursor: "pointer" }}
                      >
                        {entry.paid ? (
                          <Badge tone="ok">שולם</Badge>
                        ) : entry.endedAt ? (
                          <Badge tone="warn">לא שולם</Badge>
                        ) : (
                          <Badge tone="accent">פעיל</Badge>
                        )}
                      </button>
                    </td>
                    <td>
                      <div className="row-actions">
                        <IconButton icon="edit" label="עריכה" onClick={() => openEdit(entry)} />
                        <IconButton icon="trash" label="מחיקה" onClick={() => void remove(entry)} />
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
        {hasMore && (
          <div style={{ display: "flex", justifyContent: "center", padding: "12px 16px" }}>
            <Button variant="ghost" loading={loadingMore} onClick={() => void loadMore()}>
              טעינת עוד ({summary.total - entries.length})
            </Button>
          </div>
        )}
      </Card>

      <div className="note">
        <Icon name="info" size={16} />
        <span>
          רשומות מחויבות לפי התעריף שנשמר ברגע היצירה, כך ששינוי תעריף בהמשך לא משנה דוחות קודמים.
        </span>
      </div>

      <Modal
        open={draft !== null}
        title={draft?.id ? "עריכת רשומת זמן" : "רשומת זמן חדשה"}
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
            <Field label="פרויקט" className="span-2">
              <Select
                value={draft.projectId ?? ""}
                placeholder="ללא פרויקט"
                options={projects.map((project) => ({
                  value: project.id,
                  label: project.clientName ? `${project.name} · ${project.clientName}` : project.name,
                }))}
                onChange={(event) =>
                  setDraft({ ...draft, projectId: event.target.value || null })
                }
              />
            </Field>
            <Field label="התחלה">
              <Input
                type="datetime-local"
                value={draft.startedAt}
                onChange={(event) => setDraft({ ...draft, startedAt: event.target.value })}
              />
            </Field>
            <Field label="סיום">
              <Input
                type="datetime-local"
                value={draft.endedAt}
                onChange={(event) => setDraft({ ...draft, endedAt: event.target.value })}
              />
            </Field>
            <Field label="תיאור" className="span-2">
              <Textarea
                value={draft.description}
                onChange={(event) => setDraft({ ...draft, description: event.target.value })}
                placeholder="מה נעשה בפועל?"
              />
            </Field>
            <Field label="חיוב">
              <Select
                value={draft.billable ? "yes" : "no"}
                options={[
                  { value: "yes", label: "השעה מחויבת" },
                  { value: "no", label: "לא לחיוב" },
                ]}
                onChange={(event) => setDraft({ ...draft, billable: event.target.value === "yes" })}
              />
            </Field>
            <Field label="תשלום">
              <Select
                value={draft.paid ? "yes" : "no"}
                options={[
                  { value: "no", label: "טרם שולם" },
                  { value: "yes", label: "שולם" },
                ]}
                onChange={(event) => setDraft({ ...draft, paid: event.target.value === "yes" })}
              />
            </Field>
          </div>
        )}
      </Modal>
    </div>
  );
}
