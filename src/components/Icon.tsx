type IconName =
  | "clock"
  | "play"
  | "stop"
  | "users"
  | "folder"
  | "chart"
  | "wallet"
  | "bell"
  | "settings"
  | "user"
  | "cloud"
  | "download"
  | "plus"
  | "edit"
  | "trash"
  | "check"
  | "close"
  | "search"
  | "refresh"
  | "grid"
  | "arrow"
  | "pin"
  | "spark"
  | "drive"
  | "info";

const PATHS: Record<IconName, string> = {
  clock: "M12 7v5l3 2M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z",
  play: "M8 5.5v13l11-6.5-11-6.5Z",
  stop: "M7 7h10v10H7z",
  users: "M16 19v-1.5c0-2-1.8-3.5-4-3.5s-4 1.5-4 3.5V19M12 11a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7Zm7 8v-1.2c0-1.4-.9-2.6-2.2-3.1M5 19v-1.2c0-1.4.9-2.6 2.2-3.1",
  folder: "M3 8.5A2.5 2.5 0 0 1 5.5 6H9l2 2.5h7.5A2.5 2.5 0 0 1 21 11v6.5A2.5 2.5 0 0 1 18.5 20h-13A2.5 2.5 0 0 1 3 17.5v-9Z",
  chart: "M4 20V4m0 16h16M8 16V9m4 7V6m4 10v-4",
  wallet: "M3 8.5A2.5 2.5 0 0 1 5.5 6H19a1 1 0 0 1 1 1v10a2 2 0 0 1-2 2H6a3 3 0 0 1-3-3V8.5Zm0 0A2.5 2.5 0 0 0 5.5 11H20M16 14.5h1",
  bell: "M18 15.5V11a6 6 0 1 0-12 0v4.5L4.5 18h15L18 15.5ZM10 21h4",
  settings:
    "M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6Zm8.4-3a8.4 8.4 0 0 0-.1-1.3l2-1.5-2-3.4-2.3 1a8.4 8.4 0 0 0-2.2-1.3L13.4 3h-3.8l-.4 2.5A8.4 8.4 0 0 0 7 6.8l-2.3-1-2 3.4 2 1.5a8.4 8.4 0 0 0 0 2.6l-2 1.5 2 3.4 2.3-1a8.4 8.4 0 0 0 2.2 1.3l.4 2.5h3.8l.4-2.5a8.4 8.4 0 0 0 2.2-1.3l2.3 1 2-3.4-2-1.5c.1-.4.1-.9.1-1.3Z",
  user: "M12 12a4 4 0 1 0 0-8 4 4 0 0 0 0 8Zm7 8v-.5c0-2.5-3.1-4.5-7-4.5s-7 2-7 4.5V20",
  cloud: "M7 18h10a4 4 0 0 0 .4-8A6 6 0 0 0 6 11.5 3.5 3.5 0 0 0 7 18Zm5-3V9m0 0-2.5 2.5M12 9l2.5 2.5",
  download: "M12 4v11m0 0 4-4m-4 4-4-4M5 20h14",
  plus: "M12 5v14M5 12h14",
  edit: "M4 20h4l10-10-4-4L4 16v4Zm9-13 4 4",
  trash: "M4 7h16M9 7V4h6v3m-8 0 1 13h8l1-13",
  check: "m5 13 4 4L19 7",
  close: "M6 6l12 12M18 6 6 18",
  search: "M11 18a7 7 0 1 0 0-14 7 7 0 0 0 0 14Zm5.5-1.5L21 21",
  refresh: "M20 11a8 8 0 1 0-2.3 6M20 5v6h-6",
  grid: "M4 4h7v7H4V4Zm9 0h7v7h-7V4ZM4 13h7v7H4v-7Zm9 0h7v7h-7v-7Z",
  arrow: "M14 6l-6 6 6 6",
  pin: "M12 21s7-5.5 7-11a7 7 0 1 0-14 0c0 5.5 7 11 7 11Zm0-8.5a2.5 2.5 0 1 0 0-5 2.5 2.5 0 0 0 0 5Z",
  spark: "M12 3v4m0 10v4M3 12h4m10 0h4M6 6l2.5 2.5M15.5 15.5 18 18M18 6l-2.5 2.5M8.5 15.5 6 18",
  drive: "M9 3h6l6 10.5-3 5.5H6L3 13.5 9 3Zm0 0L6 19m9-16 3 16",
  info: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18Zm0-13v1m0 3v5",
};

export function Icon({
  name,
  size = 18,
  className,
  strokeWidth = 1.7,
}: {
  name: IconName;
  size?: number;
  className?: string;
  strokeWidth?: number;
}) {
  return (
    <svg
      className={className}
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={strokeWidth}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <path d={PATHS[name]} />
    </svg>
  );
}

export type { IconName };
