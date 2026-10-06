import { type FormEvent, useEffect, useState } from "react";
import { describe, type Api, type GameMap } from "./api";
import {
  Button,
  Card,
  cx,
  day,
  Empty,
  ErrorText,
  Field,
  Input,
  Mono,
  Page,
  Table,
} from "./ui";

/** Admins upload map files and archive them. The API does not read a file, so a bad one
 * shows as the error of the first match played on it. */
export default function Maps({ api }: { api: Api }) {
  const [maps, setMaps] = useState<GameMap[] | null>(null);
  const [error, setError] = useState("");
  // Bumped after an upload or an archive so the list refetches.
  const [version, setVersion] = useState(0);
  const refresh = () => setVersion((v) => v + 1);

  useEffect(() => {
    api.load<GameMap[]>("/maps").then(setMaps, (err) => setError(describe(err)));
  }, [version]);

  const archive = async (m: GameMap) => {
    setError("");
    try {
      await api.patch(`/maps/${encodeURIComponent(m.id)}`, {
        archived: m.archived_at === null,
      });
      refresh();
    } catch (err) {
      setError(describe(err));
    }
  };

  return (
    <Page wide>
      <div className="flex flex-col gap-6">
        <Card title="Upload a map">
          <MapUploadForm api={api} onUploaded={refresh} />
        </Card>
        <Card title="Maps" flush>
          {error && (
            <div className="p-4">
              <ErrorText>{error}</ErrorText>
            </div>
          )}
          {maps?.length === 0 && <Empty>No maps</Empty>}
          {maps && maps.length > 0 && (
            <Table head={["Name", "Game", "Size", "Uploaded", "ID", ""]}>
              {maps.map((m) => (
                <tr key={m.id} className={cx(m.archived_at && "text-muted")}>
                  <td className="px-4 py-3 font-medium">
                    {m.name}
                    {m.archived_at && (
                      <span className="ml-2 rounded border border-line px-1.5 py-px text-[11px] font-normal text-muted">
                        archived {day(m.archived_at)}
                      </span>
                    )}
                  </td>
                  <td className="px-4 py-3 text-muted">{m.game}</td>
                  <td className="px-4 py-3 text-muted tabular-nums">
                    {(m.size / 1024).toFixed(1)} KiB
                  </td>
                  <td className="px-4 py-3 whitespace-nowrap text-muted tabular-nums">
                    {day(m.created_at)}
                  </td>
                  <td className="px-4 py-3">
                    <Mono value={m.id} />
                  </td>
                  <td className="px-4 py-3 text-right">
                    <Button variant="ghost" onClick={() => archive(m)}>
                      {m.archived_at ? "Unarchive" : "Archive"}
                    </Button>
                  </td>
                </tr>
              ))}
            </Table>
          )}
        </Card>
      </div>
    </Page>
  );
}

function MapUploadForm({ api, onUploaded }: { api: Api; onUploaded: () => void }) {
  const [file, setFile] = useState<File | null>(null);
  const [name, setName] = useState("");
  const [game, setGame] = useState("ucbc2027");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    if (!file) return;
    setBusy(true);
    setError("");
    const form = new FormData();
    form.append("file", file);
    form.append("game", game);
    if (name.trim()) form.append("name", name.trim());
    try {
      await api.upload("/maps", form);
      setFile(null);
      setName("");
      (e.target as HTMLFormElement).reset();
      onUploaded();
    } catch (err) {
      setError(describe(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <form className="flex flex-col gap-4" onSubmit={submit}>
      <label
        className={cx(
          "flex cursor-pointer flex-col items-center justify-center gap-1 rounded-lg border border-dashed px-4 py-6 text-center transition-colors hover:border-fg",
          file ? "border-fg bg-panel" : "border-line",
        )}
      >
        <input
          className="sr-only"
          type="file"
          accept=".map"
          onChange={(e) => setFile(e.target.files?.[0] ?? null)}
        />
        <span className="font-medium">{file ? file.name : "Choose a .map"}</span>
        {file && (
          <span className="text-xs text-muted">{(file.size / 1024).toFixed(1)} KiB</span>
        )}
      </label>
      <div className="grid gap-3 sm:grid-cols-2">
        <Field label="Name">
          <Input
            placeholder="The file's name"
            value={name}
            onChange={(e) => setName(e.target.value)}
          />
        </Field>
        <Field label="Game">
          <Input value={game} onChange={(e) => setGame(e.target.value)} />
        </Field>
      </div>
      <div className="flex items-center gap-3">
        <Button variant="accent" disabled={!file || !game.trim() || busy}>
          {busy ? "Uploading…" : "Upload"}
        </Button>
        {error && <ErrorText>{error}</ErrorText>}
      </div>
    </form>
  );
}
