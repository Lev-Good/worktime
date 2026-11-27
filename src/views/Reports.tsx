import { useEffect, useMemo, useState } from "react";
import { api, errorText } from "../lib/api";
import { settingNumber, useApp } from "../lib/store";
import type { Report } from "../lib/types";
import { Button, Card, Empty, Field, Input, Stat } from "../components/ui";
import { BarChart } from "../components/ui";
import { Icon } from "../components/Icon";
import { fmtHM, fmtHours, fmtMoney, startOfMonthISO, startOfWeekISO, todayISO, toISODate } from "../lib/format";

type Preset = "week" | "month" | "lastMonth" | "custom";
type Group = "day" | "client" | "project";

function lastMonthRange() {
  const now = new Date();
  const first = new Date(now.getFullYear(), now.getMonth() - 1, 1);
  const last = new Date(now.getFullYear(), now.getMonth(), 0);
  return { from: toISODate(first), to: toISODate(last) };
}

export default function Reports() {
  const { settings, currency, notify, reloadKey } = useApp();
  const [preset, setPreset] = useState<Preset>("month");
  const [from, setFrom] = useState(startOfMonthISO());
  const [to, setTo] = useState(todayISO());
  const [group, setGroup] = useState<Group>("day");
  const [report, setReport] = useState<Report | null>(null);
  const [exporting, setExporting] = useState(false);

  const range = useMemo(() => {
    if (preset === "week") return { from: startOfWeekISO(), to: todayISO() };
    if (preset === "month") return { from: startOfMonthISO(), to: todayISO() };
    if (preset === "lastMonth") return lastMonthRange();
    return { from, to };
  }, [preset, from, to]);

  useEffect(() => {
    api
      .report(range.from, range.to)
      .then(setReport)
      .catch((error) => notify(errorText(error), "error"));
  }, [range.from, range.to, reloadKey, notify]);

  const dailyGoal = settingNumber(settings, "dailyGoalHours", 8) * 3600;
  const rows =
    group === "day" ? report?.byDay ?? [] : group === "client" ? report?.byClient ?? [] : report?.byProject ?? [];
  const maxSeconds = Math.max(1, ...rows.map((row) => row.seconds));

  const exportCsv = async () => {
    setExporting(true);
    try {
      const path = await api.exportReport(range.from, range.to);
      notify(`הדוח נשמר בקובץ: ${path}`);
    } catch (error) {
      notify(errorText(error), "error");
    } finally {
      setExporting(false);
    }
  };

  return (
    <div className="page">
      <div className="page-head">
        <div>
          <h1 className="page-title">דוחות וסיכומים</h1>
          <p className="page-sub">פילוח שעות והכנסות לפי יום, לקוח ופרויקט — כולל ייצוא לאקסל</p>
        </div>
        <div className="page-actions">
          <Button variant="soft" icon="download" loading={exporting} onClick={() => void exportCsv()}>
            ייצוא CSV
          </Button>
        </div>
      </div>

      <Card>
        <div className="filters">
          <div className="segmented" style={{ alignSelf: "flex-end" }}>
            {(
              [
                { value: "week", label: "השבוע" },
                { value: "month", label: "החודש" },
                { value: "lastMonth", label: "חודש קודם" },
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
                  onChange={(event) => setTo(event.target.value)}
                />
              </Field>
            </>
          )}
          <div className="spacer" />
          <span className="muted small">
            {range.from} ← {range.to}
          </span>
        </div>
      </Card>

      <div className="grid grid-4">
        <Stat label="סה״כ שעות" value={fmtHM(report?.totalSeconds ?? 0)} sub={`${report?.entryCount ?? 0} רשומות`} tone="accent" />
        <Stat
          label="שעות לחיוב"
          value={fmtHM(report?.billableSeconds ?? 0)}
          sub={`${fmtHours(report?.billableSeconds ?? 0)} שעות`}
        />
        <Stat label="סכום לחיוב" value={fmtMoney(report?.totalEarnings ?? 0, currency)} tone="ok" />
        <Stat
          label="ממתין לתשלום"
          value={fmtMoney(report?.unpaidEarnings ?? 0, currency)}
          sub={`שולם ${fmtMoney(report?.paidEarnings ?? 0, currency)}`}
          tone="warn"
        />
      </div>

      <Card
        title="פילוח"
        subtitle="בחרו חתך כדי לראות איפה הזמן והכסף נמצאים"
        actions={
          <div className="segmented">
            {(
              [
                { value: "day", label: "לפי יום" },
                { value: "client", label: "לפי לקוח" },
                { value: "project", label: "לפי פרויקט" },
              ] as { value: Group; label: string }[]
            ).map((option) => (
              <button
                key={option.value}
                type="button"
                className={group === option.value ? "is-active" : ""}
                onClick={() => setGroup(option.value)}
              >
                {option.label}
              </button>
            ))}
          </div>
        }
      >
        {rows.length === 0 ? (
          <Empty icon="chart" title="אין נתונים בטווח הזה" hint="בחרו טווח אחר או התחילו לתעד עבודה." />
        ) : (
          <div className="stack" style={{ gap: 20 }}>
            {group === "day" && (
              <BarChart
                data={rows.map((row) => ({ label: row.label, seconds: row.seconds }))}
                format={fmtHM}
                goalSeconds={dailyGoal}
              />
            )}

            <div className="table-wrap">
              <table className="table">
                <thead>
                  <tr>
                    <th>{group === "day" ? "תאריך" : group === "client" ? "לקוח" : "פרויקט"}</th>
                    <th className="num">שעות</th>
                    <th className="num">סכום</th>
                    <th className="num">חלק מהזמן</th>
                    <th>התפלגות</th>
                  </tr>
                </thead>
                <tbody>
                  {rows.map((row) => (
                    <tr key={row.key || row.label}>
                      <td className="strong">{row.label}</td>
                      <td className="num">{fmtHM(row.seconds)}</td>
                      <td className="num">{fmtMoney(row.earnings, currency)}</td>
                      <td className="num">
                        {Math.round((row.seconds / Math.max(1, report?.totalSeconds ?? 1)) * 100)}%
                      </td>
                      <td style={{ width: 160 }}>
                        <div className="progress-bar">
                          <span style={{ width: `${Math.max(2, (row.seconds / maxSeconds) * 100)}%` }} />
                        </div>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        )}
      </Card>

      <div className="note">
        <Icon name="info" size={16} />
        <span>
          הדוח מחושב לפי שעת ההתחלה של כל רשומה. קובץ ה-CSV נשמר בתיקיית WorkTime שבמסמכים, כולל
          תמיכה מלאה בעברית ובאקסל.
        </span>
      </div>
    </div>
  );
}
