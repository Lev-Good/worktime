import { useEffect, type ButtonHTMLAttributes, type InputHTMLAttributes, type ReactNode, type SelectHTMLAttributes, type TextareaHTMLAttributes } from "react";
import { Icon, type IconName } from "./Icon";

export function Card({
  children,
  title,
  subtitle,
  actions,
  className = "",
  bodyClass = "",
}: {
  children?: ReactNode;
  title?: ReactNode;
  subtitle?: ReactNode;
  actions?: ReactNode;
  className?: string;
  bodyClass?: string;
}) {
  return (
    <section className={`card ${className}`}>
      {(title || actions) && (
        <header className="card-head">
          <div>
            {title && <h2 className="card-title">{title}</h2>}
            {subtitle && <p className="card-sub">{subtitle}</p>}
          </div>
          {actions && <div className="card-actions">{actions}</div>}
        </header>
      )}
      <div className={`card-body ${bodyClass}`}>{children}</div>
    </section>
  );
}

type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: "primary" | "ghost" | "outline" | "danger" | "soft";
  size?: "sm" | "md" | "lg";
  icon?: IconName;
  loading?: boolean;
};

export function Button({
  children,
  variant = "outline",
  size = "md",
  icon,
  loading,
  className = "",
  ...rest
}: ButtonProps) {
  return (
    <button
      {...rest}
      disabled={rest.disabled || loading}
      className={`btn btn-${variant} btn-${size} ${className}`}
    >
      {loading ? <span className="spinner" /> : icon ? <Icon name={icon} size={size === "sm" ? 15 : 17} /> : null}
      {children && <span>{children}</span>}
    </button>
  );
}

export function IconButton({
  icon,
  label,
  active,
  ...rest
}: ButtonHTMLAttributes<HTMLButtonElement> & { icon: IconName; label: string; active?: boolean }) {
  return (
    <button
      {...rest}
      aria-label={label}
      title={label}
      className={`icon-btn ${active ? "is-active" : ""} ${rest.className ?? ""}`}
    >
      <Icon name={icon} size={17} />
    </button>
  );
}

export function Field({
  label,
  hint,
  children,
  className = "",
}: {
  label?: string;
  hint?: string;
  children: ReactNode;
  className?: string;
}) {
  return (
    <label className={`field ${className}`}>
      {label && <span className="field-label">{label}</span>}
      {children}
      {hint && <span className="field-hint">{hint}</span>}
    </label>
  );
}

export function Input(props: InputHTMLAttributes<HTMLInputElement>) {
  return <input {...props} className={`input ${props.className ?? ""}`} />;
}

export function Textarea(props: TextareaHTMLAttributes<HTMLTextAreaElement>) {
  return <textarea {...props} className={`input textarea ${props.className ?? ""}`} />;
}

export function Select({
  options,
  placeholder,
  ...rest
}: SelectHTMLAttributes<HTMLSelectElement> & {
  options: { value: string; label: string }[];
  placeholder?: string;
}) {
  return (
    <select {...rest} className={`input select ${rest.className ?? ""}`}>
      {placeholder && <option value="">{placeholder}</option>}
      {options.map((option) => (
        <option key={option.value} value={option.value}>
          {option.label}
        </option>
      ))}
    </select>
  );
}

export function Toggle({
  checked,
  onChange,
  label,
  hint,
  disabled,
}: {
  checked: boolean;
  onChange: (value: boolean) => void;
  label: string;
  hint?: string;
  disabled?: boolean;
}) {
  return (
    <div className="toggle-row">
      <div className="toggle-copy">
        <span className="toggle-label">{label}</span>
        {hint && <span className="field-hint">{hint}</span>}
      </div>
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        aria-label={label}
        disabled={disabled}
        onClick={() => onChange(!checked)}
        className={`switch ${checked ? "is-on" : ""}`}
      >
        <span className="switch-knob" />
      </button>
    </div>
  );
}

export function Badge({
  children,
  tone = "neutral",
}: {
  children: ReactNode;
  tone?: "neutral" | "accent" | "ok" | "warn" | "danger";
}) {
  return <span className={`badge badge-${tone}`}>{children}</span>;
}

export function Modal({
  open,
  title,
  subtitle,
  onClose,
  children,
  footer,
  wide,
}: {
  open: boolean;
  title: string;
  subtitle?: string;
  onClose: () => void;
  children: ReactNode;
  footer?: ReactNode;
  wide?: boolean;
}) {
  useEffect(() => {
    if (!open) return;
    const handler = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [open, onClose]);

  if (!open) return null;
  return (
    <div className="modal-backdrop" onMouseDown={(event) => event.target === event.currentTarget && onClose()}>
      <div className={`modal ${wide ? "modal-wide" : ""}`} role="dialog" aria-modal="true" aria-label={title}>
        <header className="modal-head">
          <div>
            <h2>{title}</h2>
            {subtitle && <p>{subtitle}</p>}
          </div>
          <IconButton icon="close" label="סגור" onClick={onClose} />
        </header>
        <div className="modal-body">{children}</div>
        {footer && <footer className="modal-foot">{footer}</footer>}
      </div>
    </div>
  );
}

export function Empty({
  icon = "spark",
  title,
  hint,
  action,
}: {
  icon?: IconName;
  title: string;
  hint?: string;
  action?: ReactNode;
}) {
  return (
    <div className="empty">
      <span className="empty-icon">
        <Icon name={icon} size={22} />
      </span>
      <strong>{title}</strong>
      {hint && <p>{hint}</p>}
      {action}
    </div>
  );
}

export function Stat({
  label,
  value,
  sub,
  tone,
}: {
  label: string;
  value: ReactNode;
  sub?: ReactNode;
  tone?: "accent" | "ok" | "warn" | "danger";
}) {
  return (
    <div className={`stat ${tone ? `stat-${tone}` : ""}`}>
      <span className="stat-label">{label}</span>
      <span className="stat-value">{value}</span>
      {sub && <span className="stat-sub">{sub}</span>}
    </div>
  );
}

export function BarChart({
  data,
  format,
  goalSeconds,
}: {
  data: { label: string; seconds: number; hint?: string }[];
  format: (seconds: number) => string;
  goalSeconds?: number;
}) {
  const max = Math.max(goalSeconds ?? 0, ...data.map((item) => item.seconds), 1);
  return (
    <div className="bars" style={{ gridTemplateColumns: `repeat(${Math.max(data.length, 1)}, minmax(0, 1fr))` }}>
      {data.map((item) => {
        const height = Math.max(4, Math.round((item.seconds / max) * 100));
        return (
          <div className="bar-col" key={item.label + item.seconds}>
            <span className={`bar-value ${item.seconds === 0 ? "is-muted" : ""}`}>{format(item.seconds)}</span>
            <div className="bar-track">
              {goalSeconds ? <div className="bar-goal" style={{ bottom: `${(goalSeconds / max) * 100}%` }} /> : null}
              <div className="bar-fill" style={{ height: `${height}%` }} />
            </div>
            <span className="bar-label">{item.label}</span>
          </div>
        );
      })}
    </div>
  );
}

export function Segmented<T extends string>({
  value,
  options,
  onChange,
}: {
  value: T;
  options: { value: T; label: string }[];
  onChange: (value: T) => void;
}) {
  return (
    <div className="segmented">
      {options.map((option) => (
        <button
          key={option.value}
          type="button"
          className={option.value === value ? "is-active" : ""}
          onClick={() => onChange(option.value)}
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}

export function ProgressRing({
  progress,
  size = 112,
  stroke = 9,
  label,
  sub,
}: {
  progress: number;
  size?: number;
  stroke?: number;
  label: ReactNode;
  sub?: ReactNode;
}) {
  const radius = (size - stroke) / 2;
  const circumference = 2 * Math.PI * radius;
  const clamped = Math.min(1, Math.max(0, progress));
  return (
    <div className="ring" style={{ width: size, height: size }}>
      <svg width={size} height={size}>
        <circle className="ring-track" cx={size / 2} cy={size / 2} r={radius} strokeWidth={stroke} />
        <circle
          className="ring-fill"
          cx={size / 2}
          cy={size / 2}
          r={radius}
          strokeWidth={stroke}
          strokeDasharray={circumference}
          strokeDashoffset={circumference * (1 - clamped)}
        />
      </svg>
      <div className="ring-label">
        <strong>{label}</strong>
        {sub && <span>{sub}</span>}
      </div>
    </div>
  );
}
