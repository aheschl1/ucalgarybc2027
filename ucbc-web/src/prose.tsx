import { type ReactNode } from "react";

export function Section({
  id,
  n,
  title,
  children,
}: {
  id: string;
  n?: number;
  title: string;
  children: ReactNode;
}) {
  return (
    <section id={id}>
      <h2>
        {n !== undefined && <span className="n">{n}</span>}
        {title}
      </h2>
      {children}
    </section>
  );
}

export function Rule({ title, children }: { title: string; children?: ReactNode }) {
  return (
    <div className="rule">
      <strong>{title}</strong>
      {children && <div>{children}</div>}
    </div>
  );
}
