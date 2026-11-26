import { useEffect, useState, type ReactNode, type RefObject } from "react";

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

export function JumpBar({
  id,
  watch,
  sections,
}: {
  id: string;
  watch: RefObject<HTMLElement | null>;
  sections: readonly (readonly [string, string])[];
}) {
  const [show, setShow] = useState(false);
  const [expanded, setExpanded] = useState(true);
  const [open, setOpen] = useState(false);

  useEffect(() => {
    const element = watch.current;
    if (!element) return;
    const observer = new IntersectionObserver(([entry]) => {
      if (entry) setShow(!entry.isIntersecting);
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, [watch]);

  if (!show) return null;

  const jump = (target: string) => {
    document.getElementById(target)?.scrollIntoView({ behavior: "smooth" });
    history.replaceState(history.state, "", `#${target}`);
    setOpen(false);
  };

  if (!expanded) {
    return (
      <button
        type="button"
        className="fixed top-16 right-4 z-30 rounded-full border border-line bg-bg px-3 py-2 text-xs font-medium shadow-md hover:bg-hover"
        aria-expanded="false"
        onClick={() => setExpanded(true)}
      >
        Jump to
      </button>
    );
  }

  return (
    <div className="fixed inset-x-0 top-14 z-30 border-b border-line bg-bg/95 px-4 py-3 shadow-sm backdrop-blur sm:px-6">
      <div className="mx-auto flex max-w-3xl items-center gap-3">
        <button
          type="button"
          className="flex items-center gap-1.5 rounded-md px-2 py-1 text-xs font-medium tracking-wide text-muted uppercase hover:bg-hover hover:text-fg"
          aria-expanded={open}
          aria-controls={id}
          onClick={() => setOpen(!open)}
        >
          Jump to <span aria-hidden>{open ? "▴" : "▾"}</span>
        </button>
        <button
          type="button"
          className="ml-auto rounded-md px-2 py-1 text-xs text-muted hover:bg-hover hover:text-fg"
          onClick={() => {
            setOpen(false);
            setExpanded(false);
          }}
        >
          Hide
        </button>
      </div>
      {open && (
        <ul id={id} className="mx-auto mt-2 flex max-w-3xl flex-col gap-0.5 text-sm">
          {sections.map(([target, title]) => (
            <li key={target}>
              <button
                type="button"
                className="block w-full rounded-md px-2 py-1 text-left text-muted hover:bg-hover hover:text-fg"
                onClick={() => jump(target)}
              >
                {title}
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
