// The standalone page: edits `map.bin`, which `ucbc editor` serves and saves with PUT.
// Without it the page starts on a new map, and with no file to save to (`ucbc editor`
// alone), Save downloads the map instead.

import { createEditor } from "./editor.ts";

const MAP = "map.bin";

async function save(bytes: Uint8Array) {
  // Copied: fetch and Blob take only a Uint8Array over a plain ArrayBuffer.
  const body = new Uint8Array(bytes);
  const r = await fetch(MAP, { method: "PUT", body });
  if (r.status === 404) return download(body);
  if (!r.ok) throw new Error(`${r.status} ${r.statusText}`);
}

function download(body: Uint8Array<ArrayBuffer>) {
  const a = document.createElement("a");
  a.href = URL.createObjectURL(new Blob([body]));
  a.download = "map.map";
  a.click();
  URL.revokeObjectURL(a.href);
  return "Downloaded map.map";
}

async function load() {
  const r = await fetch(MAP);
  const body = r.ok ? new Uint8Array(await r.arrayBuffer()) : null;
  // An empty file is a new map.
  if (body && body.byteLength > 0) editor.open(body);
}

const editor = createEditor(document.getElementById("app")!, { save });
load().catch((e) => alert(`${MAP} is not a map file: ${e instanceof Error ? e.message : e}`));
