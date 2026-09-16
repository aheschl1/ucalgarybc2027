export class ApiError extends Error {
  constructor(
    readonly status: number,
    detail: string,
  ) {
    super(detail);
  }
}

// The session is a same-origin cookie, which fetch sends on its own.
async function send<T>(path: string, init: RequestInit = {}): Promise<T> {
  const res = await fetch(`/api${path}`, init);
  if (!res.ok) {
    const body = await res.json().catch(() => null);
    const detail = typeof body?.detail === "string" ? body.detail : res.statusText;
    throw new ApiError(res.status, detail);
  }
  return res.status === 204 ? (undefined as T) : res.json();
}

export function get<T>(path: string): Promise<T> {
  return send(path);
}

export function post<T>(path: string, body: unknown): Promise<T> {
  return send(path, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
}

export function upload<T>(path: string, form: FormData): Promise<T> {
  return send(path, { method: "POST", body: form });
}

export function logIn(username: string, password: string): Promise<User> {
  return post("/auth/login", { username, password });
}

export function logOut(): Promise<void> {
  return post("/auth/logout", undefined);
}

export type User = {
  id: number;
  username: string;
  is_admin: boolean;
  created_at: string;
};

// Mirrors SetResult and TeamInfo in ucbc-engine.
export type SetResult = {
  index: number;
  first_team: number;
  winner_team: number | null;
  reason: "win" | "draw" | "forfeit";
  detail: string;
  ticks: number;
};

export type Submission = {
  id: string;
  user_id: number;
  username: string;
  name: string;
  game: string;
  size: number;
  sha256: string;
  created_at: string;
};

export type BotSource = { kind: "path"; path: string } | { kind: "submission"; id: string };

export type Match = {
  id: string;
  origin: "user" | "platform";
  game: string;
  bots: BotSource[];
  engine_version: string | null;
  teams: { id: number; name: string }[];
  status: "queued" | "running" | "done" | "error";
  set_wins: number[] | null;
  winner_team: number | null;
  error: string | null;
  created_at: string;
  sets?: SetResult[];
};
