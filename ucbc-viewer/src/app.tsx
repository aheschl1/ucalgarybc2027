// The standalone page: loads `?replay=<url>` (default `replay.json`, which `ucbc view`
// serves), or a replay file, gzipped or not, dropped onto the page. With neither, it lists
// the sample replays the dev server offers (vite/replays.ts).

import { StrictMode, type ReactNode } from "react";
import { createRoot } from "react-dom/client";

import { renderers } from "./games.ts";
import { parseReplay } from "./load.ts";
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

async function open(stream: ReadableStream<Uint8Array>, source: string) {
  // A replay dropped while another loads replaces it.
  const id = ++opened;
  show(`Loading ${source}…`);
  let replay: Replay;
  try {
    replay = (await parseReplay(stream)) as Replay;
  } catch (e) {
    if (id === opened) show(e instanceof SyntaxError ? `${source} is not JSON.` : `${source} could not be read.`);
    return;
  }
  if (id !== opened) return;
  if (!Array.isArray(replay?.sets) || !replay.sets.length || typeof replay.config?.game !== "string") {
    return show(`${source} is not a replay.`);
  }
  render(<Viewer key={id} replay={replay} renderers={renderers} />);
}

const url = new URLSearchParams(location.search).get("replay") ?? "replay.json";
fetch(url)
  .then((r) => (r.ok && r.body ? r.body : Promise.reject()))
  .then((body) => open(body, url))
  .catch(async () => {
    const list = await samples();
    show(list.length ? "Drop a replay file here, or open a sample:" : "Drop a replay file here.", list);
  });

addEventListener("dragover", (e) => e.preventDefault());
/** A replay file's bytes, decompressed if it is gzipped (`ucbc run --replay r.json.gz`). */
async function readFile(file: File): Promise<ReadableStream<Uint8Array>> {
  const [a, b] = new Uint8Array(await file.slice(0, 2).arrayBuffer());
  if (a !== 0x1f || b !== 0x8b) return file.stream();
  return file.stream().pipeThrough(new DecompressionStream("gzip"));
}

addEventListener("drop", async (e) => {
  e.preventDefault();
  const file = e.dataTransfer?.files[0];
  if (file) open(await readFile(file), file.name);
});
