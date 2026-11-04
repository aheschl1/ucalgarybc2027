import { parseReplay, renderers, replayStream, Viewer, type Replay } from "@ucbc/viewer";
import sample from "@ucbc/viewer-ucbc2027/replays/standard.json.gz?url";
import { pictures, SkinContext, withFiles, type Picture } from "@ucbc/viewer-ucbc2027/skin";
import { useEffect, useMemo, useState, type ChangeEvent, type DragEvent, type FormEvent } from "react";
import { describe, type Api, type MatchRow } from "./api";
import { fromZip, isPng, loadOverrides, matchIdFrom, saveOverrides, toDataUrl, toZip, type Overrides } from "./skinFiles";
import { Button, buttonClass, Card, cx, day, ErrorText, Input, Select } from "./ui";
import { matchReplay } from "./Viewer";

const GAME = "ucbc2027";
const PICTURES = pictures();
const FILES = new Set(PICTURES.map((p) => p.file));

const GROUPS: [string, RegExp][] = [
  ["Tiles and items", /^(empty|wall|fossil)/],
  ["Labs", /^lab\./],
  ["Team A dinos", /^dino\.0/],
  ["Team B dinos", /^dino\.1/],
  ["Not in the game yet", /^unused/],
  ["Other", /./],
];

const GROUPED = (() => {
  const titled = new Map<string, Picture[]>(GROUPS.map(([title]) => [title, []]));
  for (const p of PICTURES) titled.get(GROUPS.find(([, re]) => re.test(p.slots[0]!))![0])!.push(p);
  return [...titled].filter(([, members]) => members.length);
})();

async function sampleReplay(): Promise<Replay> {
  const response = await fetch(sample);
  if (!response.ok) throw new Error(response.statusText);
  return (await parseReplay(await replayStream(await response.blob()))) as Replay;
}

function matchName(match: MatchRow): string {
  return `${match.teams.map((t) => t.name).join(" vs ")} · ${day(match.created_at)} · ${match.id.slice(0, 8)}`;
}

function imageSize(href: string): Promise<[number, number]> {
  return new Promise((resolve, reject) => {
    const image = new Image();
    image.onload = () => resolve([image.naturalWidth, image.naturalHeight]);
    image.onerror = reject;
    image.src = href;
  });
}

function pictureName({ file, slots }: Picture): string {
  const dino = slots[0]!.match(/^dino\.\d\.(\d)\.(\d)$/);
  if (dino) return `Tier ${Number(dino[1]) + 1}, frame ${Number(dino[2]) + 1}`;
  return file.split("/").at(-1)!.replace(".png", "");
}

function plural(n: number, noun: string): string {
  return `${n} ${noun}${n === 1 ? "" : "s"}`;
}

function PictureRow({
  picture,
  override,
  onFile,
  onReset,
}: {
  picture: Picture;
  override?: string;
  onFile: (file: File) => void;
  onReset: () => void;
}) {
  const [over, setOver] = useState(false);
  const [sizes, setSizes] = useState<{ override: string; text: string | null } | null>(null);

  useEffect(() => {
    if (!override) return;
    let live = true;
    Promise.all([imageSize(picture.href), imageSize(override)]).then(
      ([[w, h], [newW, newH]]) => {
        const text = w === newW && h === newH ? null : `${newW}×${newH}, but the original is ${w}×${h}`;
        if (live) setSizes({ override, text });
      },
      () => {},
    );
    return () => {
      live = false;
    };
  }, [override, picture.href]);

  const drop = (e: DragEvent) => {
    e.preventDefault();
    setOver(false);
    const file = e.dataTransfer.files[0];
    if (file) onFile(file);
  };
  const warning = override && sizes?.override === override ? sizes.text : null;

  return (
    <li
      onDragOver={(e) => {
        e.preventDefault();
        setOver(true);
      }}
      onDragLeave={() => setOver(false)}
      onDrop={drop}
      className={cx("px-4 py-2 transition-colors", over && "bg-accent-soft")}
    >
      <div className="flex items-center gap-3">
        <label className="flex min-w-0 flex-1 cursor-pointer items-center gap-3 rounded-lg hover:text-accent has-focus-visible:ring-2 has-focus-visible:ring-accent">
          <input
            className="sr-only"
            type="file"
            accept="image/png"
            onChange={(e) => {
              const file = e.target.files?.[0];
              e.target.value = "";
              if (file) onFile(file);
            }}
          />
          <img src={override ?? picture.href} alt="" className="pixel size-11 shrink-0 rounded border border-line bg-panel" />
          <span className="min-w-0">
            <span className="block truncate text-sm font-medium">
              {pictureName(picture)}
              {override && <span className="ml-2 text-xs font-normal text-accent">changed</span>}
            </span>
            <span className="block truncate font-mono text-xs text-muted">{picture.file}</span>
          </span>
        </label>
        {override && (
          <Button variant="ghost" onClick={onReset}>
            Reset
          </Button>
        )}
      </div>
      {warning && <p className="mt-1 text-xs text-accent">{warning}</p>}
    </li>
  );
}

export default function Skins({ api }: { api: Api }) {
  const [replay, setReplay] = useState<Replay | null>(null);
  const [overrides, setOverrides] = useState<Overrides>({});
  const [matchId, setMatchId] = useState<string | null>(null);
  const [attempt, setAttempt] = useState(0);
  const [recent, setRecent] = useState<MatchRow[]>([]);
  const [pasted, setPasted] = useState("");
  const [matchError, setMatchError] = useState("");
  const [error, setError] = useState("");
  const [note, setNote] = useState("");
  const [unsaved, setUnsaved] = useState(false);

  useEffect(() => {
    let live = true;
    api.load<MatchRow[]>("/matches?origin=platform&done=true").then(
      (rows) => live && setRecent(rows.filter((m) => m.game === GAME)),
      (err) => live && setError(`Recent matches could not be loaded: ${describe(err)}`),
    );
    loadOverrides().then(
      (stored) => {
        const known = Object.entries(stored).filter(([file]) => FILES.has(file));
        if (live) setOverrides((current) => ({ ...Object.fromEntries(known), ...current }));
      },
      () => live && setUnsaved(true),
    );
    const refuse = (e: Event) => e.preventDefault();
    addEventListener("dragover", refuse);
    addEventListener("drop", refuse);
    return () => {
      live = false;
      removeEventListener("dragover", refuse);
      removeEventListener("drop", refuse);
    };
  }, []);

  useEffect(() => {
    let live = true;
    setReplay(null);
    setMatchError("");
    (matchId ? matchReplay(api, matchId) : sampleReplay()).then(
      (result) => {
        if (!live) return;
        if (typeof result === "string") setMatchError(`This match has no replay yet (${result}).`);
        else if (result.config.game !== GAME) setMatchError(`This is a ${result.config.game} match, and these pictures are for ${GAME}.`);
        else setReplay(result);
      },
      (err) => live && setMatchError(matchId ? describe(err) : "The sample match could not be loaded."),
    );
    return () => {
      live = false;
    };
  }, [matchId, attempt]);

  const skin = useMemo(() => withFiles(overrides), [overrides]);
  const changed = Object.keys(overrides).length;

  const apply = (changes: Record<string, string | null>) => {
    setOverrides((current) => {
      const next = { ...current };
      for (const [file, href] of Object.entries(changes)) {
        if (href === null) delete next[file];
        else next[file] = href;
      }
      return next;
    });
    saveOverrides(changes).catch(() => setUnsaved(true));
  };

  const loadPasted = (e: FormEvent) => {
    e.preventDefault();
    const id = matchIdFrom(pasted);
    if (!id) return setError("That doesn't contain a match ID.");
    setError("");
    setMatchId(id);
    setAttempt((n) => n + 1);
  };

  const replace = async (picture: Picture, file: File) => {
    setError("");
    setNote("");
    const bytes = await file.arrayBuffer().then((b) => new Uint8Array(b), () => new Uint8Array());
    const href = toDataUrl(bytes);
    const readable = isPng(bytes) && (await imageSize(href).then(() => true, () => false));
    if (!readable) return setError(`${file.name} is not a readable PNG.`);
    apply({ [picture.file]: href });
  };

  const importZip = async (e: ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    e.target.value = "";
    if (!file) return;
    setError("");
    setNote("");
    let unzipped: ReturnType<typeof fromZip>;
    try {
      unzipped = fromZip(new Uint8Array(await file.arrayBuffer()), FILES);
    } catch {
      return setError(`${file.name} is not a zip.`);
    }
    const checked = await Promise.all(
      Object.entries(unzipped.found).map(async ([name, href]) => [name, href, await imageSize(href).then(() => true, () => false)] as const),
    );
    const good = checked.filter(([, , ok]) => ok);
    const skipped = [...unzipped.skipped, ...checked.filter(([, , ok]) => !ok).map(([name]) => name)];
    if (good.length) apply(Object.fromEntries(good.map(([name, href]) => [name, href])));
    setNote(`Imported ${plural(good.length, "picture")}.${skipped.length ? ` Skipped ${skipped.join(", ")}.` : ""}`);
  };

  const exportZip = () => {
    const url = URL.createObjectURL(new Blob([toZip(overrides) as BlobPart], { type: "application/zip" }));
    const link = document.createElement("a");
    link.href = url;
    link.download = "ucbc2027-skin.zip";
    link.click();
    URL.revokeObjectURL(url);
  };

  const resetAll = () => {
    if (!window.confirm(`Reset ${plural(changed, "picture")} to the original art?`)) return;
    setNote("");
    apply(Object.fromEntries(Object.keys(overrides).map((file) => [file, null])));
  };

  return (
    <main className="mx-auto w-full max-w-[96rem] px-4 py-6 sm:px-6">
      <header className="mb-4 flex flex-wrap items-end justify-between gap-4">
        <div className="min-w-0">
          <h1 className="text-2xl font-semibold tracking-tight">Skin editor</h1>
          <p className="mt-1 text-sm text-muted">
            Replace any picture and the match redraws wherever it appears, though a match may not show them all. Changes stay in this browser until you export them; send the zip to a developer to add to the game.
          </p>
        </div>
        <div className="flex flex-wrap gap-2">
          <Button variant="ghost" disabled={!changed} onClick={resetAll}>
            Reset all
          </Button>
          <label className={cx(buttonClass("secondary"), "has-focus-visible:ring-2 has-focus-visible:ring-accent")}>
            <input className="sr-only" type="file" accept=".zip,application/zip" onChange={importZip} />
            Import zip
          </label>
          <Button disabled={!changed} onClick={exportZip}>
            {changed ? `Export ${plural(changed, "change")}` : "Export"}
          </Button>
        </div>
      </header>

      {(error || note || unsaved) && (
        <div className="mb-4 space-y-1">
          {error && <ErrorText>{error}</ErrorText>}
          {note && <p className="text-sm text-muted">{note}</p>}
          {unsaved && (
            <ErrorText>This browser isn't keeping your changes, so export them before you leave the page.</ErrorText>
          )}
        </div>
      )}

      <div className="grid items-start gap-6 xl:grid-cols-[minmax(0,1fr)_22rem]">
        <div className="min-w-0">
          <div className="mb-4 flex flex-wrap gap-2">
            <Select
              aria-label="Match"
              className="min-w-56 flex-1"
              value={matchId ?? ""}
              onChange={(e) => {
                setError("");
                setPasted("");
                setMatchId(e.target.value || null);
              }}
            >
              <option value="">Sample match</option>
              {matchId && !recent.some((m) => m.id === matchId) && <option value={matchId}>Match {matchId.slice(0, 8)}</option>}
              {recent.map((m) => (
                <option key={m.id} value={m.id}>
                  {matchName(m)}
                </option>
              ))}
            </Select>
            <form onSubmit={loadPasted} className="flex min-w-56 flex-1 gap-2">
              <Input
                aria-label="Match ID or link"
                placeholder="Paste a match ID or link"
                value={pasted}
                onChange={(e) => setPasted(e.target.value)}
              />
              <Button type="submit" variant="secondary" disabled={!pasted.trim()}>
                Load
              </Button>
            </form>
          </div>
          {matchError && <div className="mb-4"><ErrorText>{matchError}</ErrorText></div>}
          {replay ? (
            <SkinContext value={skin}>
              <Viewer replay={replay} renderers={renderers} autoFocus={false} />
            </SkinContext>
          ) : (
            !matchError && <p className="animate-pulse py-24 text-center text-muted">Loading match…</p>
          )}
        </div>
        <Card flush title="Pictures" className="xl:sticky xl:top-20">
          <div className="overflow-y-auto xl:max-h-[calc(100vh-10rem)]">
            {GROUPED.map(([title, members]) => (
              <section key={title}>
                <h3 className="border-y border-line bg-panel px-4 py-1.5 text-xs font-medium text-muted">{title}</h3>
                <ul>
                  {members.map((picture) => (
                    <PictureRow
                      key={picture.file}
                      picture={picture}
                      override={overrides[picture.file]}
                      onFile={(file) => replace(picture, file)}
                      onReset={() => apply({ [picture.file]: null })}
                    />
                  ))}
                </ul>
              </section>
            ))}
          </div>
        </Card>
      </div>
    </main>
  );
}
