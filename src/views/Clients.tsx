import { useEffect, useMemo, useState } from "react";
import { api, errorText } from "../lib/api";
import { useApp } from "../lib/store";
import type { Client, ClientInput } from "../lib/types";
import { Badge, Button, Card, Empty, Field, IconButton, Input, Modal, Textarea } from "../components/ui";
import { Icon } from "../components/Icon";
import { fmtHM, fmtMoney } from "../lib/format";

const EMPTY: ClientInput = {
  id: null,
  name: "",
  contact: "",
  email: "",
  phone: "",
  address: "",
  notes: "",
  hourlyRate: null,
  archived: false,
};

export default function Clients() {
  const { currency, notify, reloadKey, projects } = useApp();
  const [items, setItems] = useState<Client[]>([]);
  const [search, setSearch] = useState("");
  const [draft, setDraft] = useState<ClientInput | null>(null);
  const [saving, setSaving] = useState(false);

  const load = () => {
    api
      .clients()
      .then(setItems)
      .catch((error) => notify(errorText(error), "error"));
  };

  useEffect(load, [reloadKey]);

  const filtered = useMemo(() => {
    const query = search.trim().toLowerCase();
    if (!query) return items;
    return items.filter((client) =>
      [client.name, client.contact, client.email, client.phone]
        .join(" ")
        .toLowerCase()
        .includes(query)
    );
  }, [items, search]);

  const save = async () => {
    if (!draft) return;
    if (!draft.name.trim()) {
      notify("יש להזין שם לקוח", "error");
      return;
    }
    setSaving(true);
    try {
      await api.saveClient({ ...draft, name: draft.name.trim() });
      notify("הלקוח נשמר");
      setDraft(null);
      load();
    } catch (error) {
      notify(errorText(error), "error");
    } finally {
      setSaving(false);
    }
  };

  const remove = async (client: Client) => {
    if (!window.confirm(`למחוק את הלקוח "${client.name}"? הפרויקטים והרשומות יישארו ללא שיוך ללקוח.`))
      return;
    try {
      await api.deleteClient(client.id);
      notify("הלקוח נמחק");
      load();
    } catch (error) {
      notify(errorText(error), "error");
    }
  };

  const active = items.filter((client) => !client.archived);
  const unpaid = items.reduce((total, client) => total + client.unpaidAmount, 0);

  return (
    <div className="page">
      <div className="page-head">
        <div>
          <h1 className="page-title">לקוחות</h1>
          <p className="page-sub">
            {active.length} לקוחות פעילים · {fmtMoney(unpaid, currency)} ממתין לתשלום
          </p>
        </div>
        <div className="page-actions">
          <div style={{ position: "relative" }}>
            <Input
              placeholder="חיפוש לקוח…"
              value={search}
              onChange={(event) => setSearch(event.target.value)}
              style={{ width: 220, paddingInlineStart: 34 }}
            />
            <span
              style={{ position: "absolute", insetInlineStart: 11, top: 11, color: "var(--muted)" }}
            >
              <Icon name="search" size={16} />
            </span>
          </div>
          <Button variant="primary" icon="plus" onClick={() => setDraft({ ...EMPTY })}>
            לקוח חדש
          </Button>
        </div>
      </div>

      {filtered.length === 0 ? (
        <Card>
          <Empty
            icon="users"
            title={search ? "לא נמצאו לקוחות" : "עוד אין לקוחות"}
            hint={
              search
                ? "נסו חיפוש אחר."
                : "הוסיפו לקוח כדי לשייך פרויקטים, שעות וחיובים — ותמיד לדעת מי חייב כמה."
            }
            action={
              !search ? (
                <Button variant="primary" icon="plus" onClick={() => setDraft({ ...EMPTY })}>
                  הוספת לקוח
                </Button>
              ) : undefined
            }
          />
        </Card>
      ) : (
        <div className="grid grid-3">
          {filtered.map((client) => {
            const projectCount = projects.filter((project) => project.clientId === client.id).length;
            return (
              <Card key={client.id} className="card-hero">
                <div className="row" style={{ alignItems: "flex-start" }}>
                  <span
                    className="avatar"
                    style={{ background: "linear-gradient(140deg,#6d8dff,#4b5bd6)" }}
                  >
                    {client.name.slice(0, 1)}
                  </span>
                  <div className="list-main">
                    <strong>{client.name}</strong>
                    <span>{client.contact || client.email || client.phone || "ללא פרטי קשר"}</span>
                  </div>
                  {client.archived && <Badge tone="neutral">ארכיון</Badge>}
                </div>

                <div className="kv" style={{ marginTop: 14 }}>
                  <dt>סה״כ שעות</dt>
                  <dd>{fmtHM(client.totalSeconds)}</dd>
                  <dt>ממתין לתשלום</dt>
                  <dd>{fmtMoney(client.unpaidAmount, currency)}</dd>
                  <dt>פרויקטים</dt>
                  <dd>{projectCount}</dd>
                  <dt>תעריף</dt>
                  <dd>
                    {client.hourlyRate ? `${fmtMoney(client.hourlyRate, currency)}/שעה` : "לפי הפרופיל"}
                  </dd>
                </div>

                <div className="row" style={{ marginTop: 16, justifyContent: "flex-end" }}>
                  <IconButton
                    icon="edit"
                    label="עריכה"
                    onClick={() =>
                      setDraft({
                        id: client.id,
                        name: client.name,
                        contact: client.contact,
                        email: client.email,
                        phone: client.phone,
                        address: client.address,
                        notes: client.notes,
                        hourlyRate: client.hourlyRate,
                        archived: client.archived,
                      })
                    }
                  />
                  <IconButton icon="trash" label="מחיקה" onClick={() => void remove(client)} />
                </div>
              </Card>
            );
          })}
        </div>
      )}

      <Modal
        open={draft !== null}
        title={draft?.id ? "עריכת לקוח" : "לקוח חדש"}
        subtitle="פרטי הקשר והתעריף ישמשו כברירת מחדל לפרויקטים ולחיובים."
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
            <Field label="שם הלקוח" className="span-2">
              <Input
                autoFocus
                value={draft.name}
                onChange={(event) => setDraft({ ...draft, name: event.target.value })}
                placeholder="לדוגמה: סטודיו ברק"
              />
            </Field>
            <Field label="איש קשר">
              <Input
                value={draft.contact}
                onChange={(event) => setDraft({ ...draft, contact: event.target.value })}
              />
            </Field>
            <Field label="טלפון">
              <Input
                value={draft.phone}
                onChange={(event) => setDraft({ ...draft, phone: event.target.value })}
                placeholder="050-0000000"
              />
            </Field>
            <Field label="אימייל">
              <Input
                type="email"
                value={draft.email}
                onChange={(event) => setDraft({ ...draft, email: event.target.value })}
              />
            </Field>
            <Field label="תעריף שעתי" hint="ריק = לפי תעריף הפרופיל">
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
            <Field label="כתובת" className="span-2">
              <Input
                value={draft.address}
                onChange={(event) => setDraft({ ...draft, address: event.target.value })}
              />
            </Field>
            <Field label="הערות" className="span-2">
              <Textarea
                value={draft.notes}
                onChange={(event) => setDraft({ ...draft, notes: event.target.value })}
                placeholder="סיכומים, תנאי תשלום, בקשות מיוחדות…"
              />
            </Field>
          </div>
        )}
      </Modal>
    </div>
  );
}
