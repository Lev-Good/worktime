export const CURRENCY_SYMBOL: Record<string, string> = {
  ILS: "₪",
  USD: "$",
  EUR: "€",
  GBP: "£",
};

export const CURRENCY_OPTIONS = [
  { value: "ILS", label: "שקל (₪)" },
  { value: "USD", label: "דולר ($)" },
  { value: "EUR", label: "אירו (€)" },
  { value: "GBP", label: "לירה שטרלינג (£)" },
];

export function symbolOf(currency: string): string {
  return CURRENCY_SYMBOL[currency] ?? "₪";
}

export function fmtMoney(value: number, currency = "ILS"): string {
  const symbol = symbolOf(currency);
  const rounded = Math.round(value * 100) / 100;
  const formatted = new Intl.NumberFormat("he-IL", {
    maximumFractionDigits: Number.isInteger(rounded) ? 0 : 2,
    minimumFractionDigits: 0,
  }).format(rounded);
  return `${formatted} ${symbol}`;
}

export function fmtClock(totalSeconds: number, withSeconds = true): string {
  const seconds = Math.max(0, Math.floor(totalSeconds));
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const rest = seconds % 60;
  const pad = (value: number) => String(value).padStart(2, "0");
  return withSeconds ? `${pad(hours)}:${pad(minutes)}:${pad(rest)}` : `${pad(hours)}:${pad(minutes)}`;
}

export function fmtHM(totalSeconds: number): string {
  const minutes = Math.max(0, Math.round(totalSeconds / 60));
  const hours = Math.floor(minutes / 60);
  return `${hours}:${String(minutes % 60).padStart(2, "0")}`;
}

export function fmtHours(totalSeconds: number, digits = 1): string {
  return (Math.max(0, totalSeconds) / 3600).toFixed(digits);
}

export function fmtDate(value: string | null | undefined): string {
  if (!value) return "—";
  const date = value.slice(0, 10).split("-");
  if (date.length !== 3) return value;
  return `${date[2]}/${date[1]}/${date[0]}`;
}

export function fmtTime(value: string | null | undefined): string {
  if (!value) return "—";
  return value.slice(11, 16);
}

export function fmtDateTime(value: string | null | undefined): string {
  if (!value) return "—";
  return `${fmtDate(value)} · ${fmtTime(value)}`;
}

export function fmtDayName(value: string): string {
  const names = ["יום ראשון", "יום שני", "יום שלישי", "יום רביעי", "יום חמישי", "יום שישי", "שבת"];
  const date = new Date(`${value}T12:00:00`);
  return names[date.getDay()] ?? value;
}

export function toISODate(date: Date): string {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

export function todayISO(): string {
  return toISODate(new Date());
}

export function startOfWeekISO(): string {
  const now = new Date();
  const copy = new Date(now);
  copy.setDate(now.getDate() - now.getDay());
  return toISODate(copy);
}

export function startOfMonthISO(): string {
  const now = new Date();
  return `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}-01`;
}

export function shiftDays(iso: string, days: number): string {
  const date = new Date(`${iso}T12:00:00`);
  date.setDate(date.getDate() + days);
  return toISODate(date);
}

export function toInputDateTime(iso: string): string {
  return iso ? iso.slice(0, 16) : "";
}

export function nowInputDateTime(): string {
  const now = new Date();
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${toISODate(now)}T${pad(now.getHours())}:${pad(now.getMinutes())}`;
}

export function normalizeDateTime(value: string): string {
  if (!value) return "";
  return `${value.slice(0, 16).replace(" ", "T")}:00`;
}

export function relativeDay(value: string): string {
  const today = todayISO();
  const day = value.slice(0, 10);
  if (day === today) return "היום";
  if (day === shiftDays(today, -1)) return "אתמול";
  return fmtDate(day);
}

export function clampNumber(value: string, fallback: number): number {
  const parsed = Number(value.replace(",", "."));
  return Number.isFinite(parsed) ? parsed : fallback;
}
