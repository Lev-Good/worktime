import { useEffect, useMemo, useState } from "react";
import { api, errorText } from "../lib/api";
import { useApp } from "../lib/store";
import type { Payment, PaymentInput } from "../lib/types";
import { Badge, Button, Card, Empty, Field, IconButton, Input, Modal, Select, Stat, Textarea, Toggle } from "../components/ui";
import { fmtDate, fmtHM, fmtMoney, nowInputDateTime, normalizeDateTime, todayISO } from "../lib/format";

const METHODS = [
  { value: "bank", label: "העברה בנקאית" },
  { value: "cash", label: "מזומן" },
  { value: "credit", label: "כרטיס אשראי" },
  { value: "bit", label: "ביט / פייבוקס" },
  { value: "check", label: "צ׳ק" },
  { value: "other", label: "אחר" },
];

function methodLabel(value: string): string {
  return METHODS.find((method) => method.value === value)?.label ?? (value || "—");
}

export default function Payments() {
  const { currency, clients, projects, notify, reloadKey } = useApp();
  const [items, setItems] = useState<Payment[]>([]);
  const [draft, setDraft] = useState<PaymentInput | null>(null);
  const [saving, setSaving] = useState(false);

  const load = () => {
    api
      .payments()
      .then(setItems)
      .catch((error) => notify(errorText(error), "error"));
  };

  useEffect(load, [reloadKey]);

  const month = todayISO().slice(0, 7);
  const thisMonth = items.filter((payment) => payment.paidAt.slice(0, 7) === month);
  const total = items.reduce((sum, payment) => sum + payment.amount, 0);
  const monthTotal = thisMonth.reduce((sum, payment) => sum + payment.amount, 0);
  const unpaidByClient = useMemo(
    () => clients.filter((client) => client.unpaidAmount > 0).sort((a, b) => b.unpaidAmount - a.unpaidAmount),
    [clients]
  );
  const unpaidTotal = unpaidByClient.reduce((sum, client) => sum + client.unpaidAmount, 0);

  const save = async () => {
    if (!draft) return;
    if (!Number.isFinite(draft.amount) || draft.amount <= 0) {
      notify("יש להזין סכום חיובי", "error");
      return;
    }
    setSaving(true);
    try {
      await api.savePayment({ ...draft, paidAt: normalizeDateTime(draft.paidAt) });
      notify("התשלום נרשם");
      setDraft(null);
      load();
    } catch (error) {
      notify(errorText(error), "error");
    } finally {
      setSaving(false);
    }
  };

  const remove = async (payment: Payment) => {
    if (!window.confirm("למחוק את רשומת התשלום?")) return;
    try {
      await api.deletePayment(payment.id);
      notify("התשלום נמחק");
      load();
    } catch (error) {
      notify(errorText(error), "error");
    }
  };

  const openNew = (clientId?: string) =>
    setDraft({
      id: null,
      clientId: clientId ?? null,
      projectId: null,
      amount: 0,
      method: "bank",
      note: "",
      paidAt: nowInputDateTime(),
      markEntriesPaid: Boolean(clientId),
    });

  return (
    <div className="page">
      <div className="page-head">
        <div>
          <h1 className="page-title">תשלומים</h1>
          <p className="page-sub">מעקב אחרי הכנסות, סגירת חיובים פתוחים והיסטוריית תשלומים</p>
        </div>
        <div className="page-actions">
          <Button variant="primary" icon="plus" onClick={() => openNew()}>
            רישום תשלום
          </Button>
        </div>
      </div>

      <div className="grid grid-3">
        <Stat label="סה״כ התקבל" value={fmtMoney(total, currency)} sub={`${items.length} תשלומים`} tone="ok" />
        <Stat
          label="התקבל החודש"
          value={fmtMoney(monthTotal, currency)}
          sub={new Date().toLocaleDateString("he-IL", { month: "long", year: "numeric" })}
        />
        <Stat
          label="חובות פתוחים"
          value={fmtMoney(unpaidTotal, currency)}
          sub={`${unpaidByClient.length} לקוחות`}
          tone={unpaidTotal > 0 ? "warn" : undefined}
        />
      </div>

      <div className="grid grid-2">
        <Card title="תשלומים אחרונים" bodyClass="flush">
          {items.length === 0 ? (
            <Empty
              icon="wallet"
              title="עוד לא נרשמו תשלומים"
              hint="כשמתקבל כסף מלקוח, רשמו אותו כאן ואפשר לסמן אילו שעות נסגרו."
              action={
                <Button variant="soft" icon="plus" onClick={() => openNew()}>
                  רישום תשלום
                </Button>
              }
            />
          ) : (
            <div className="table-wrap">
              <table className="table">
                <thead>
                  <tr>
                    <th>תאריך</th>
                    <th>לקוח / פרויקט</th>
                    <th>אמצעי</th>
                    <th className="num">סכום</th>
                    <th />
                  </tr>
                </thead>
                <tbody>
                  {items.map((payment) => (
                    <tr key={payment.id}>
                      <td className="nowrap">{fmtDate(payment.paidAt)}</td>
                      <td>
                        <div className="cell-title">
                          <span className="strong">{payment.clientName ?? "ללא לקוח"}</span>
                          <small>{payment.projectName ?? "ללא פרויקט"}</small>
                        </div>
                      </td>
                      <td>{methodLabel(payment.method)}</td>
                      <td className="num strong">{fmtMoney(payment.amount, payment.currency)}</td>
                      <td>
                        <div className="row-actions">
                          <IconButton icon="trash" label="מחיקה" onClick={() => void remove(payment)} />
                        </div>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </Card>

        <Card title="חובות פתוחים" subtitle="לפי לקוח, לפי שעות שטרם שולמו">
          {unpaidByClient.length === 0 ? (
            <Empty icon="check" title="אין חובות פתוחים" hint="כל השעות המחויבות סומנו כשולמו." />
          ) : (
            <div className="list">
              {unpaidByClient.map((client) => (
                <div className="list-row" key={client.id}>
                  <span className="avatar" style={{ background: "linear-gradient(140deg,#ffb86b,#ff758f)" }}>
                    {client.name.slice(0, 1)}
                  </span>
                  <div className="list-main">
                    <strong>{client.name}</strong>
                    <span>{fmtHM(client.totalSeconds)} שעות מתועדות</span>
                  </div>
                  <div className="list-meta">
                    <strong>{fmtMoney(client.unpaidAmount, currency)}</strong>
                    <Badge tone="warn">פתוח</Badge>
                  </div>
                  <Button size="sm" variant="soft" onClick={() => openNew(client.id)}>
                    סגירה
                  </Button>
                </div>
              ))}
            </div>
          )}
        </Card>
      </div>

      <Modal
        open={draft !== null}
        title="רישום תשלום"
        subtitle="אפשר לסמן אוטומטית את כל השעות המחויבות של הלקוח או הפרויקט כשולמו."
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
            <Field label="לקוח">
              <Select
                value={draft.clientId ?? ""}
                placeholder="ללא לקוח"
                options={clients.map((client) => ({ value: client.id, label: client.name }))}
                onChange={(event) => setDraft({ ...draft, clientId: event.target.value || null })}
              />
            </Field>
            <Field label="פרויקט">
              <Select
                value={draft.projectId ?? ""}
                placeholder="ללא פרויקט"
                options={projects
                  .filter((project) => !draft.clientId || project.clientId === draft.clientId)
                  .map((project) => ({ value: project.id, label: project.name }))}
                onChange={(event) => setDraft({ ...draft, projectId: event.target.value || null })}
              />
            </Field>
            <Field label={`סכום (${currency})`}>
              <Input
                type="number"
                min={0}
                step="1"
                autoFocus
                value={draft.amount || ""}
                onChange={(event) => setDraft({ ...draft, amount: Number(event.target.value) })}
              />
            </Field>
            <Field label="אמצעי תשלום">
              <Select
                value={draft.method}
                options={METHODS}
                onChange={(event) => setDraft({ ...draft, method: event.target.value })}
              />
            </Field>
            <Field label="תאריך תשלום">
              <Input
                type="datetime-local"
                value={draft.paidAt}
                onChange={(event) => setDraft({ ...draft, paidAt: event.target.value })}
              />
            </Field>
            <Field label="הערה">
              <Textarea
                value={draft.note}
                onChange={(event) => setDraft({ ...draft, note: event.target.value })}
                placeholder="מספר אסמכתא, תנאי תשלום…"
                style={{ minHeight: 60 }}
              />
            </Field>
            <div className="span-2">
              <Toggle
                checked={draft.markEntriesPaid}
                onChange={(value) => setDraft({ ...draft, markEntriesPaid: value })}
                label="לסמן את השעות המחויבות כשולמו"
                hint="מעדכן את כל הרשומות המתאימות של הלקוח/פרויקט שנבחרו"
              />
            </div>
          </div>
        )}
      </Modal>
    </div>
  );
}
