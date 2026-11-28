import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  Bootstrap,
  Client,
  ClientInput,
  Dashboard,
  DriveFile,
  DriveStatus,
  EntriesPage,
  EntryInput,
  NotificationItem,
  Payment,
  PaymentInput,
  Profile,
  ProfileInput,
  Project,
  ProjectInput,
  Report,
  Settings,
  TimerSnapshot,
  UpdateInfo,
} from "./types";

export { listen };

export const api = {
  bootstrap: () => invoke<Bootstrap>("bootstrap"),
  dashboard: () => invoke<Dashboard>("dashboard"),

  clients: () => invoke<Client[]>("list_clients"),
  saveClient: (client: ClientInput) => invoke<Client>("save_client", { client }),
  deleteClient: (id: string) => invoke<void>("delete_client", { id }),

  projects: () => invoke<Project[]>("list_projects"),
  saveProject: (project: ProjectInput) => invoke<Project>("save_project", { project }),
  deleteProject: (id: string) => invoke<void>("delete_project", { id }),

  entries: (params: {
    from?: string;
    to?: string;
    clientId?: string | null;
    projectId?: string | null;
    search?: string | null;
    limit?: number;
    offset?: number;
  }) => invoke<EntriesPage>("list_entries", params),
  startTimer: (projectId: string | null, description = "") =>
    invoke<TimerSnapshot>("start_timer", { projectId, description }),
  stopTimer: () => invoke<TimerSnapshot>("stop_timer"),
  saveEntry: (entry: EntryInput) => invoke<void>("save_entry", { entry }),
  deleteEntry: (id: string) => invoke<void>("delete_entry", { id }),
  setEntryPaid: (id: string, paid: boolean) => invoke<void>("set_entry_paid", { id, paid }),

  report: (from: string, to: string) => invoke<Report>("report", { from, to }),
  exportReport: (from: string, to: string) =>
    invoke<string>("export_report_csv", { from, to }),

  payments: () => invoke<Payment[]>("list_payments"),
  savePayment: (payment: PaymentInput) => invoke<void>("save_payment", { payment }),
  deletePayment: (id: string) => invoke<void>("delete_payment", { id }),

  notifications: () => invoke<NotificationItem[]>("list_notifications"),
  markNotification: (id: string, read: boolean) =>
    invoke<void>("mark_notification", { id, read }),
  clearNotifications: () => invoke<void>("clear_notifications"),

  saveSettings: (settings: Settings) => invoke<Settings>("save_settings", { settings }),
  saveProfile: (profile: ProfileInput) => invoke<Profile>("save_profile", { profile }),
  deleteProfile: (id: string) => invoke<void>("delete_profile", { id }),
  switchProfile: (id: string) => invoke<void>("switch_profile", { id }),

  setFloating: (visible: boolean) => invoke<boolean>("set_floating", { visible }),
  timer: () => invoke<TimerSnapshot>("timer"),

  checkUpdates: (force = false) => invoke<UpdateInfo>("check_updates", { force }),
  openUrl: (url: string) => invoke<void>("open_url", { url }),
  backupLocal: () => invoke<string>("backup_local"),
  showMain: (view?: string) => invoke<void>("show_main", { view: view ?? null }),
  quit: () => invoke<void>("quit_app"),

  driveStatus: () => invoke<DriveStatus>("gdrive_status"),
  driveConnect: (clientId: string, clientSecret: string) =>
    invoke<DriveStatus>("gdrive_connect", { clientId, clientSecret }),
  driveDisconnect: () => invoke<void>("gdrive_disconnect"),
  driveBackup: () => invoke<DriveStatus>("gdrive_backup"),
  driveFiles: () => invoke<DriveFile[]>("gdrive_backups"),
  driveRestore: (fileId: string) => invoke<void>("gdrive_restore", { fileId }),
};

export function errorText(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return "אירעה שגיאה לא צפויה";
}
