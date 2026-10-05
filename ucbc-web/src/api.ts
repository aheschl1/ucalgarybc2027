export class ApiError extends Error {
  constructor(
    readonly status: number,
    detail: string,
  ) {
    super(detail);
  }
}

// FastAPI sends a string for our own errors and a list of field problems for a body it
// could not parse; a form needs to show either.
function detail(body: unknown): string | null {
  const value = (body as { detail?: unknown } | null)?.detail;
  if (typeof value === "string") return value;
  if (Array.isArray(value)) {
    const msgs = value
      .map((p) => (typeof p?.msg === "string" ? p.msg : ""))
      .filter(Boolean);
    return msgs.length > 0 ? msgs.join("; ") : null;
  }
  return null;
}

// The session is a same-origin cookie, which fetch sends on its own.
async function send<T>(path: string, init: RequestInit = {}): Promise<T> {
  const res = await fetch(`/api${path}`, init);
  if (!res.ok) {
    const body = await res.json().catch(() => null);
    throw new ApiError(res.status, detail(body) ?? res.statusText);
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

export function logIn(email: string, password: string): Promise<User> {
  return post("/auth/login", { email, password });
}

export function logOut(): Promise<void> {
  return post("/auth/logout", undefined);
}

export function signUp(
  email: string,
  displayName: string,
  password: string,
): Promise<User> {
  return post("/users", { email, display_name: displayName, password });
}

export type Api = {
  load: <T>(path: string) => Promise<T>;
  post: <T>(path: string, body: unknown) => Promise<T>;
  upload: <T>(path: string, form: FormData) => Promise<T>;
};

/** The calls a signed-in page makes. A 401 means the session went away, so it logs out. */
export function makeApi(onLogOut: () => void): Api {
  const guard = async <T>(call: Promise<T>): Promise<T> => {
    try {
      return await call;
    } catch (err) {
      if (err instanceof ApiError && err.status === 401) onLogOut();
      throw err;
    }
  };
  return {
    load: (path) => guard(get(path)),
    post: (path, body) => guard(post(path, body)),
    upload: (path, form) => guard(upload(path, form)),
  };
}

export function describe(err: unknown): string {
  return err instanceof ApiError
    ? `${err.status} ${err.message}`
    : "could not reach the api";
}

export type User = {
  id: number;
  email: string;
  display_name: string;
  is_admin: boolean;
  team_id: number;
  created_at: string;
};

// A participant team, not the engine's in-game teams. Only its members are sent one.
export type Team = {
  id: number;
  name: string;
  join_code: string;
  members: string[];
  created_at: string;
};

// A team's rating, rounded. Anyone may read every team's.
export type TeamElo = {
  id: number;
  name: string;
  elo: number;
  matches: number;
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
  display_name: string;
  team_id: number;
  team_name: string;
  name: string;
  game: string;
  size: number;
  sha256: string;
  created_at: string;
};

export type Match = {
  id: string;
  origin: "user" | "platform";
  game: string;
  bots: string[];
  engine_version: string | null;
  teams: { id: number; name: string }[];
  config: { sets: number; seed: number; step_ms: number; memory_bytes: number };
  status: "queued" | "running" | "done" | "error";
  set_wins: number[] | null;
  winner_team: number | null;
  error: string | null;
  created_at: string;
  sets?: SetResult[];
};
