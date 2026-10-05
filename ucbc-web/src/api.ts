import type { User } from "./api.gen";

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

export function patch<T>(path: string, body: unknown): Promise<T> {
  return send(path, {
    method: "PATCH",
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
  patch: <T>(path: string, body: unknown) => Promise<T>;
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
    patch: (path, body) => guard(patch(path, body)),
    upload: (path, form) => guard(upload(path, form)),
  };
}

export function describe(err: unknown): string {
  return err instanceof ApiError
    ? `${err.status} ${err.message}`
    : "could not reach the api";
}

// The API's own models, generated from its OpenAPI schema by `make api-types`.
export type {
  Map as GameMap,
  Match,
  MatchRow,
  MyTeam as Team,
  SetResult,
  Submission,
  TeamElo,
  User,
} from "./api.gen";
