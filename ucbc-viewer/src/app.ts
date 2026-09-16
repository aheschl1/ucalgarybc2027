// The standalone page: loads `?replay=<url>` (default `replay.json`, which `ucbc view`
// serves), or a replay file dropped onto the page.

import { renderers } from "./games.ts";
import type { Replay } from "./replay.gen.ts";
import { createViewer } from "./shell.ts";

const app = document.getElementById("app")!;
const message = document.createElement("p");
message.className = "ucbc-app-message";
message.hidden = true;
const viewer = createViewer(app, { renderers });
const root = app.querySelector<HTMLElement>(".ucbc-viewer")!;
root.hidden = true;
app.append(message);

function show(text: string) {
  message.textContent = text;
  message.hidden = false;
}

function open(text: string, source: string) {
  let replay: Replay;
  try {
    replay = JSON.parse(text);
  } catch {
    return show(`${source} is not JSON.`);
  }
  if (!Array.isArray(replay?.sets) || typeof replay.config?.game !== "string") {
    return show(`${source} is not a replay.`);
  }
  message.hidden = true;
  root.hidden = false;
  viewer.load(replay);
  root.focus();
}

const url = new URLSearchParams(location.search).get("replay") ?? "replay.json";
fetch(url)
  .then((r) => (r.ok ? r.text() : Promise.reject()))
  .then((text) => open(text, url))
  .catch(() => show("Drop a replay file here."));

addEventListener("dragover", (e) => e.preventDefault());
addEventListener("drop", async (e) => {
  e.preventDefault();
  const file = e.dataTransfer?.files[0];
  if (file) open(await file.text(), file.name);
});
