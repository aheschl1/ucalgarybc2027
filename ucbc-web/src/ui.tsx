import {
  useState,
  type ButtonHTMLAttributes,
  type InputHTMLAttributes,
  type ReactNode,
  type SelectHTMLAttributes,
} from "react";
import { Link } from "react-router";
import type { MatchRow } from "./api";

/** Joins class names, skipping the falsy ones. */
export function cx(...names: (string | false | null | undefined)[]): string {
  return names.filter(Boolean).join(" ");
}

/** A page's width and gutters, its left edge in line with the nav's; `wide` for the ones
 * with tables side by side. `title` heads it, with `subtitle` under and `action` across. */
export function Page({
  wide,
  title,
  subtitle,
  action,
  children,
}: {
  wide?: boolean;
  title?: ReactNode;
  subtitle?: ReactNode;
  action?: ReactNode;
  children: ReactNode;
}) {
  return (
    <main className="mx-auto w-full max-w-6xl animate-rise px-4 py-8 sm:px-6 sm:py-12">
      <div className={cx(!wide && "max-w-3xl")}>
        {title && (
          <header className="mb-8 flex flex-wrap items-end justify-between gap-4">
            <div className="min-w-0">
              <h1 className="text-3xl font-semibold tracking-tight">{title}</h1>
              {subtitle && <p className="mt-1.5 text-muted">{subtitle}</p>}
            </div>
            {action}
          </header>
        )}
        {children}
      </div>
    </main>
  );
}

const button = {
  primary: "bg-fg text-bg hover:bg-fg/85",
  accent: "bg-accent text-accent-fg hover:bg-accent/90",
  secondary: "border border-line bg-bg hover:bg-hover",
  ghost: "text-muted hover:bg-hover hover:text-fg",
};

export const buttonClass = (variant: keyof typeof button = "primary") =>
  cx(
    "inline-flex h-9 shrink-0 cursor-pointer items-center justify-center gap-1.5 rounded-lg px-3.5 text-sm font-medium whitespace-nowrap transition active:scale-[0.98] disabled:cursor-default disabled:opacity-40 disabled:active:scale-100",
    button[variant],
  );

export function Button({
  variant = "primary",
  className,
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & { variant?: keyof typeof button }) {
  return <button {...props} className={cx(buttonClass(variant), className)} />;
}

const field =
  "h-9 w-full min-w-0 rounded-lg border border-line bg-bg px-3 text-sm placeholder:text-muted/70 transition-colors hover:border-muted/50 focus:border-fg focus:outline-none focus-visible:outline-none disabled:opacity-50";

export function Input({ className, ...props }: InputHTMLAttributes<HTMLInputElement>) {
  return <input {...props} className={cx(field, className)} />;
}

export function Select({ className, ...props }: SelectHTMLAttributes<HTMLSelectElement>) {
  return <select {...props} className={cx(field, "pr-8", className)} />;
}

/** A labelled field; the label sits above, small and muted. */
export function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <label className="flex min-w-0 flex-col gap-1.5">
      <span className="text-xs font-medium text-muted">{label}</span>
      {children}
    </label>
  );
}

/** One of a few options, all in view. */
export function Segmented<K extends string>({
  value,
  options,
  onChange,
  label,
}: {
  value: K;
  options: readonly (readonly [K, string])[];
  onChange: (k: K) => void;
  label: string;
}) {
  return (
    <div role="radiogroup" aria-label={label} className="inline-flex h-9 rounded-lg bg-hover p-0.5">
      {options.map(([k, text]) => (
        <button
          key={k}
          type="button"
          role="radio"
          aria-checked={k === value}
          onClick={() => onChange(k)}
          className={cx(
            "cursor-pointer rounded-md px-3 text-xs font-medium transition",
            k === value ? "bg-bg text-fg shadow-sm ring-1 ring-line" : "text-muted hover:text-fg",
          )}
        >
          {text}
        </button>
      ))}
    </div>
  );
}

/** A bordered panel. `flush` drops the body's padding, for a table edge to edge. */
export function Card({
  title,
  action,
  flush,
  className,
  children,
}: {
  title?: ReactNode;
  action?: ReactNode;
  flush?: boolean;
  className?: string;
  children: ReactNode;
}) {
  return (
    <section className={cx("min-w-0 overflow-hidden rounded-xl border border-line bg-bg", className)}>
      {(title || action) && (
        <header className="flex min-h-12 flex-wrap items-center justify-between gap-3 border-b border-line px-4 py-2.5">
          {title && <h2 className="flex items-center gap-2 font-semibold">{title}</h2>}
          {action}
        </header>
      )}
      <div className={flush ? "" : "p-4"}>{children}</div>
    </section>
  );
}

/** A count beside a card's title. */
export function Count({ n }: { n: number }) {
  return (
    <span className="rounded-full bg-hover px-1.5 font-mono text-[11px] font-medium text-muted tabular-nums">
      {n}
    </span>
  );
}

/** A table that scrolls sideways inside its card rather than widening the page. */
export function Table({ head, children }: { head: ReactNode[]; children: ReactNode }) {
  return (
    <div className="overflow-x-auto">
      <table className="w-full text-left text-sm">
        <thead>
          <tr className="border-b border-line bg-panel text-[11px] tracking-wide text-muted uppercase">
            {head.map((h, i) => (
              <th key={i} className="px-4 py-2 font-medium whitespace-nowrap">
                {h}
              </th>
            ))}
          </tr>
        </thead>
        <tbody className="divide-y divide-line">{children}</tbody>
      </table>
    </div>
  );
}

/** What a list shows with nothing in it: a line, and a hint at what to do. */
export function Empty({ children, hint }: { children: ReactNode; hint?: ReactNode }) {
  return (
    <div className="px-4 py-10 text-center">
      <p className="font-medium">{children}</p>
      {hint && <p className="mt-1 text-muted">{hint}</p>}
    </div>
  );
}

/** Rows' worth of grey bars while a list loads. */
export function Loading({ rows = 3 }: { rows?: number }) {
  return (
    <div className="flex flex-col gap-3 p-4" aria-label="Loading">
      {Array.from({ length: rows }, (_, i) => (
        <div
          key={i}
          className="h-4 animate-pulse rounded bg-hover"
          style={{ width: `${90 - i * 18}%` }}
        />
      ))}
    </div>
  );
}

export function ErrorText({ children }: { children: ReactNode }) {
  return <p className="text-sm text-accent">{children}</p>;
}

const pill: Record<MatchRow["status"], string> = {
  queued: "text-muted",
  running: "text-fg",
  done: "text-muted",
  error: "text-accent",
};
const dot: Record<MatchRow["status"], string> = {
  queued: "border border-muted",
  running: "bg-accent animate-pulse",
  done: "bg-muted/60",
  error: "bg-accent",
};

export function StatusPill({ status }: { status: MatchRow["status"] }) {
  return (
    <span className={cx("inline-flex items-center gap-1.5 text-xs font-medium", pill[status])}>
      <span className={cx("size-1.5 rounded-full", dot[status])} />
      {status}
    </span>
  );
}

/** Text that copies to the clipboard on click, saying so for a moment. */
export function useCopy(): [boolean, (text: string) => void] {
  const [copied, setCopied] = useState(false);
  const copy = (text: string) =>
    navigator.clipboard.writeText(text).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1200);
    });
  return [copied, copy];
}

/** An id, cut short; clicking copies the whole of it. */
export function Mono({ value, short = 8 }: { value: string; short?: number }) {
  const [copied, copy] = useCopy();
  return (
    <button
      type="button"
      title={copied ? "copied" : `${value} (click to copy)`}
      onClick={(e) => {
        e.stopPropagation();
        copy(value);
      }}
      className="cursor-pointer rounded font-mono text-xs text-muted hover:text-fg"
    >
      {copied ? "copied" : value.length > short ? value.slice(0, short) : value}
    </button>
  );
}

/** A link styled as text that underlines on hover. */
export function TextLink({ to, children }: { to: string; children: ReactNode }) {
  return (
    <Link to={to} className="font-medium underline-offset-4 hover:underline">
      {children}
    </Link>
  );
}

/** A right-pointing chevron, turned down when `open`. */
export function Chevron({ open }: { open?: boolean }) {
  return (
    <svg
      viewBox="0 0 16 16"
      className={cx("size-3.5 shrink-0 text-muted transition-transform", open && "rotate-90")}
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
    >
      <path d="M6 4l4 4-4 4" />
    </svg>
  );
}

/** A file picked by click or dropped on it. */
export function Dropzone({
  file,
  accept,
  hint,
  onFile,
}: {
  file: File | null;
  accept: string;
  hint?: string;
  onFile: (file: File | null) => void;
}) {
  const [over, setOver] = useState(false);
  return (
    <label
      onDragOver={(e) => {
        e.preventDefault();
        setOver(true);
      }}
      onDragLeave={() => setOver(false)}
      onDrop={(e) => {
        e.preventDefault();
        setOver(false);
        onFile(e.dataTransfer.files[0] ?? null);
      }}
      className={cx(
        "flex cursor-pointer items-center gap-3 rounded-lg border border-dashed px-4 py-4 transition-colors hover:border-fg",
        over ? "border-accent bg-accent-soft" : file ? "border-fg bg-panel" : "border-line",
      )}
    >
      <input
        className="sr-only"
        type="file"
        accept={accept}
        onChange={(e) => onFile(e.target.files?.[0] ?? null)}
      />
      <span className="grid size-9 shrink-0 place-items-center rounded-lg border border-line bg-bg text-muted">
        <svg viewBox="0 0 20 20" className="size-4" fill="none" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" strokeLinejoin="round">
          {file ? <path d="M4 10.5l4 4 8-9" /> : <path d="M10 13V4m0 0L6.5 7.5M10 4l3.5 3.5M4 13v2.5h12V13" />}
        </svg>
      </span>
      <span className="min-w-0">
        <span className="block truncate font-medium">
          {file ? file.name : `Drop a ${accept} or click to choose`}
        </span>
        <span className="block text-xs text-muted">
          {file ? `${(file.size / 1024).toFixed(1)} KiB` : hint}
        </span>
      </span>
    </label>
  );
}

/** The date part of an ISO timestamp. */
export function day(iso: string): string {
  return iso.slice(0, 10);
}

/** How long ago an ISO timestamp was, roughly; past a week, the date. */
export function ago(iso: string): string {
  const s = (Date.now() - Date.parse(iso)) / 1000;
  if (s < 60) return "just now";
  if (s < 3600) return `${Math.floor(s / 60)}m ago`;
  if (s < 86400) return `${Math.floor(s / 3600)}h ago`;
  if (s < 7 * 86400) return `${Math.floor(s / 86400)}d ago`;
  return day(iso);
}

/** A timestamp as `ago`, the full one on hover. */
export function Time({ iso }: { iso: string }) {
  return (
    <time dateTime={iso} title={new Date(iso).toLocaleString()} className="whitespace-nowrap tabular-nums">
      {ago(iso)}
    </time>
  );
}
