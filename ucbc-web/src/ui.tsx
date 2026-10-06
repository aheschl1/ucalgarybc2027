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
 * with tables side by side. */
export function Page({
  wide,
  title,
  children,
}: {
  wide?: boolean;
  title?: string;
  children: ReactNode;
}) {
  return (
    <main className="mx-auto w-full max-w-6xl px-4 py-8 sm:px-6 sm:py-10">
      <div className={cx(!wide && "max-w-3xl")}>
        {title && <h1 className="mb-6 text-2xl font-semibold tracking-tight">{title}</h1>}
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

export function Button({
  variant = "primary",
  className,
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & { variant?: keyof typeof button }) {
  return (
    <button
      {...props}
      className={cx(
        "inline-flex h-9 shrink-0 cursor-pointer items-center justify-center gap-1.5 rounded-lg px-3.5 text-sm font-medium whitespace-nowrap transition-colors disabled:cursor-default disabled:opacity-40",
        button[variant],
        className,
      )}
    />
  );
}

const field =
  "h-9 w-full min-w-0 rounded-lg border border-line bg-bg px-3 text-sm placeholder:text-muted/70 transition-colors hover:border-muted/50 focus:border-fg focus:outline-none focus-visible:outline-none";

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
    <section className={cx("min-w-0 rounded-xl border border-line bg-bg", className)}>
      {(title || action) && (
        <header className="flex min-h-12 flex-wrap items-center justify-between gap-3 border-b border-line px-4 py-2.5">
          {title && <h2 className="font-semibold">{title}</h2>}
          {action}
        </header>
      )}
      <div className={flush ? "" : "p-4"}>{children}</div>
    </section>
  );
}

/** A table that scrolls sideways inside its card rather than widening the page. */
export function Table({ head, children }: { head: ReactNode[]; children: ReactNode }) {
  return (
    <div className="overflow-x-auto">
      <table className="w-full text-left text-sm">
        <thead>
          <tr className="border-b border-line text-xs text-muted">
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

export function Empty({ children }: { children: ReactNode }) {
  return <p className="px-4 py-8 text-center text-muted">{children}</p>;
}

export function ErrorText({ children }: { children: ReactNode }) {
  return <p className="text-sm text-accent">{children}</p>;
}

const pill: Record<MatchRow["status"], string> = {
  queued: "text-muted",
  running: "text-fg",
  done: "text-fg",
  error: "text-accent",
};
const dot: Record<MatchRow["status"], string> = {
  queued: "bg-muted/50",
  running: "bg-accent animate-pulse",
  done: "bg-fg",
  error: "bg-accent",
};

export function StatusPill({ status }: { status: MatchRow["status"] }) {
  return (
    <span
      className={cx(
        "inline-flex items-center gap-1.5 rounded-full border border-line px-2 py-0.5 text-xs font-medium",
        pill[status],
      )}
    >
      <span className={cx("size-1.5 rounded-full", dot[status])} />
      {status}
    </span>
  );
}

/** An id, cut short; clicking copies the whole of it. */
export function Mono({ value, short = 8 }: { value: string; short?: number }) {
  const [copied, setCopied] = useState(false);
  const copy = () =>
    navigator.clipboard.writeText(value).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1200);
    });
  return (
    <button
      type="button"
      title={copied ? "copied" : `${value} (click to copy)`}
      onClick={(e) => {
        e.stopPropagation();
        copy();
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

/** The date part of an ISO timestamp. */
export function day(iso: string): string {
  return iso.slice(0, 10);
}
