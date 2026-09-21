import { type ReactNode } from "react";
import Markdown, { type Components } from "react-markdown";
import { Link } from "react-router";
import source from "./docs.md?raw";

// The contents list in docs.md links to these, so a heading's id is its text.
const slug = (children: ReactNode) =>
  String(children).trim().toLowerCase().replace(/\s+/g, "-");

const components: Components = {
  h2: ({ children }) => <h2 id={slug(children)}>{children}</h2>,
  h3: ({ children }) => <h3 id={slug(children)}>{children}</h3>,
  // Links into the app stay client-side.
  a: ({ href, children }) =>
    href?.startsWith("/") ? (
      <Link to={href}>{children}</Link>
    ) : (
      <a href={href}>{children}</a>
    ),
};

export default function Docs() {
  return (
    <main className="page docs">
      <Markdown components={components}>{source}</Markdown>
      <p className="back">
        <Link to="/">back</Link>
      </p>
    </main>
  );
}
