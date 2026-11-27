import { createContext, useContext } from "react";
import type {
  Client,
  NotificationItem,
  Profile,
  Project,
  Settings,
  TimerSnapshot,
} from "./types";

export type View =
  | "dashboard"
  | "entries"
  | "clients"
  | "projects"
  | "reports"
  | "payments"
  | "notifications"
  | "profiles"
  | "settings";

export type AppStore = {
  version: string;
  settings: Settings;
  currency: string;
  profiles: Profile[];
  activeProfileId: string;
  timer: TimerSnapshot;
  projects: Project[];
  clients: Client[];
  notifications: NotificationItem[];
  reloadKey: number;
  refreshAll: () => Promise<void>;
  notify: (text: string, tone?: "ok" | "error") => void;
  navigate: (view: View) => void;
};

export const AppContext = createContext<AppStore | null>(null);

export function useApp(): AppStore {
  const store = useContext(AppContext);
  if (!store) throw new Error("AppContext is missing");
  return store;
}

export function settingString(settings: Settings, key: string, fallback = ""): string {
  const value = settings[key];
  return value === undefined || value === null ? fallback : value;
}

export function settingNumber(settings: Settings, key: string, fallback: number): number {
  const value = Number(String(settings[key] ?? "").replace(",", "."));
  return Number.isFinite(value) ? value : fallback;
}

export function settingBoolean(settings: Settings, key: string, fallback = false): boolean {
  const value = settings[key];
  if (value === undefined) return fallback;
  return value === "true" || value === "1";
}
