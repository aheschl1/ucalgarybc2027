// The standalone page: loads `?replay=<url>` (default `replay.json`, which `ucbc view`
// serves), or a replay file, gzipped or not, dropped onto the page. With neither, it lists
// the sample replays the dev server offers (vite/replays.ts).

import { StrictMode, type ReactNode } from "react";
import { createRoot } from "react-dom/client";

import { renderers } from "./games.ts";
import type { Replay } from "./replay.gen.ts";
import { Viewer } from "./viewer.tsx";

const root = createRoot(document.getElementById("app")!);
// Each opened replay gets a new key, so the viewer starts it from the beginning.
let opened = 0;

function render(node: ReactNode) {
  root.render(<StrictMode>{node}</StrictMode>);
}

function show(text: string, samples: string[] = []) {
  render(
    <div className="ucbc-app-message">
      <p>{text}</p>
      {samples.length ? (
        <ul>
          {samples.map((s) => (
            <li key={s}>
              <a href={`?replay=replays/${s}`}>{s}</a>
            </li>
          ))}
        </ul>
      ) : null}
    </div>,
  );
}

/** The sample replays the dev server lists; none elsewhere. */
async function samples(): Promise<string[]> {
  const r = await fetch("replays/").catch(() => null);
  return r?.ok ? r.json().catch(() => []) : [];
}

function open(text: string, source: string) {
  let replay: Replay;
  try {
    replay = JSON.parse(text);
  } catch {
    return show(`${source} is not JSON.`);
  }
  if (!Array.isArray(replay?.sets) || !replay.sets.length || typeof replay.config?.game !== "string") {
    return show(`${source} is not a replay.`);
  }
  render(<Viewer key={++opened} replay={replay} renderers={renderers} />);
}

const url = new URLSearchParams(location.search).get("replay") ?? "replay.json";
fetch(url)
  .then((r) => (r.ok ? r.text() : Promise.reject()))
  .then((text) => open(text, url))
  .catch(async () => {
    const list = await samples();
    show(list.length ? "Drop a replay file here, or open a sample:" : "Drop a replay file here.", list);
  });

addEventListener("dragover", (e) => e.preventDefault());
/** A replay file's text, decompressed if it is gzipped (`ucbc run --replay r.json.gz`). */
async function readFile(file: File): Promise<string> {
  const [a, b] = new Uint8Array(await file.slice(0, 2).arrayBuffer());
  if (a !== 0x1f || b !== 0x8b) return file.text();
  return new Response(file.stream().pipeThrough(new DecompressionStream("gzip"))).text();
}

addEventListener("drop", async (e) => {
  e.preventDefault();
  const file = e.dataTransfer?.files[0];
  if (file) open(await readFile(file), file.name);
});
