export type Profile = {
  id: string;
  name: string;
  hourlyRate: number;
  currency: string;
  color: string;
  isActive: boolean;
  createdAt: string;
  totalSeconds: number;
};

export type Client = {
  id: string;
  profileId: string;
  name: string;
  contact: string;
  email: string;
  phone: string;
  address: string;
  notes: string;
  hourlyRate: number | null;
  archived: boolean;
  createdAt: string;
  totalSeconds: number;
  unpaidAmount: number;
};

export type Project = {
  id: string;
  profileId: string;
  clientId: string | null;
  clientName: string | null;
  name: string;
  description: string;
  status: string;
  hourlyRate: number | null;
  budget: number | null;
  color: string;
  archived: boolean;
  createdAt: string;
  totalSeconds: number;
  unpaidAmount: number;
  lastUsedAt: string | null;
};

export type Entry = {
  id: string;
  profileId: string;
  projectId: string | null;
  projectName: string | null;
  clientId: string | null;
  clientName: string | null;
  description: string;
  startedAt: string;
  endedAt: string | null;
  durationSec: number;
  rate: number;
  amount: number;
  billable: boolean;
  paid: boolean;
};

export type EntriesPage = {
  entries: Entry[];
  total: number;
  totalSeconds: number;
  totalAmount: number;
  hasMore: boolean;
};

export type Payment = {
  id: string;
  profileId: string;
  clientId: string | null;
  clientName: string | null;
  projectId: string | null;
  projectName: string | null;
  amount: number;
  currency: string;
  method: string;
  note: string;
  paidAt: string;
  createdAt: string;
};

export type NotificationItem = {
  id: string;
  title: string;
  body: string;
  kind: string;
  createdAt: string;
  read: boolean;
};

export type TimerSnapshot = {
  active: Entry | null;
  elapsedSec: number;
  running: boolean;
  projectName: string | null;
  clientName: string | null;
  description: string;
  rate: number;
};

export type SummaryRow = {
  key: string;
  label: string;
  seconds: number;
  earnings: number;
  color: string | null;
};

export type Report = {
  from: string;
  to: string;
  totalSeconds: number;
  billableSeconds: number;
  totalEarnings: number;
  paidEarnings: number;
  unpaidEarnings: number;
  entryCount: number;
  byDay: SummaryRow[];
  byClient: SummaryRow[];
  byProject: SummaryRow[];
  entries: Entry[];
};

export type DayStat = { date: string; label: string; seconds: number };

export type Dashboard = {
  todaySeconds: number;
  weekSeconds: number;
  monthSeconds: number;
  todayEarnings: number;
  weekEarnings: number;
  unpaidTotal: number;
  paidTotal: number;
  entriesToday: number;
  activeProjects: number;
  activeClients: number;
  weekDays: DayStat[];
};

export type Settings = Record<string, string>;

export type Bootstrap = {
  version: string;
  settings: Settings;
  profiles: Profile[];
  activeProfileId: string;
  timer: TimerSnapshot;
  floatingVisible: boolean;
  autostartEnabled: boolean;
};

export type UpdateAsset = { name: string; url: string; size: number };

export type UpdateInfo = {
  currentVersion: string;
  latestVersion: string;
  hasUpdate: boolean;
  title: string;
  notes: string;
  pageUrl: string;
  publishedAt: string;
  assets: UpdateAsset[];
};

export type DriveFile = { id: string; name: string; createdAt: string; size: number };

export type DriveStatus = {
  configured: boolean;
  connected: boolean;
  account: string;
  folderId: string;
  lastBackupAt: string;
  files: DriveFile[];
};

export type ClientInput = {
  id?: string | null;
  name: string;
  contact: string;
  email: string;
  phone: string;
  address: string;
  notes: string;
  hourlyRate: number | null;
  archived: boolean;
};

export type ProjectInput = {
  id?: string | null;
  name: string;
  clientId: string | null;
  description: string;
  status: string;
  hourlyRate: number | null;
  budget: number | null;
  color: string;
  archived: boolean;
};

export type EntryInput = {
  id?: string | null;
  projectId: string | null;
  description: string;
  startedAt: string;
  endedAt: string;
  billable: boolean;
  paid: boolean;
};

export type PaymentInput = {
  id?: string | null;
  clientId: string | null;
  projectId: string | null;
  amount: number;
  method: string;
  note: string;
  paidAt: string;
  markEntriesPaid: boolean;
};

export type ProfileInput = {
  id?: string | null;
  name: string;
  hourlyRate: number;
  currency?: string;
  color?: string;
};
