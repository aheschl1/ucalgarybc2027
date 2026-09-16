export class ApiError extends Error {
  constructor(
    readonly status: number,
    detail: string,
  ) {
    super(detail);
  }
}

export function basicToken(username: string, password: string): string {
  const bytes = new TextEncoder().encode(`${username}:${password}`);
  return btoa(String.fromCharCode(...bytes));
}

export async function get<T>(path: string, token: string): Promise<T> {
  const res = await fetch(`/api${path}`, { headers: { Authorization: `Basic ${token}` } });
  if (!res.ok) {
    const body = await res.json().catch(() => null);
    const detail = typeof body?.detail === "string" ? body.detail : res.statusText;
    throw new ApiError(res.status, detail);
  }
  return res.json();
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

export type Match = {
  id: string;
  game: string;
  engine_version: string | null;
  teams: { id: number; name: string }[];
  status: "queued" | "running" | "done" | "error";
  set_wins: number[] | null;
  winner_team: number | null;
  error: string | null;
  created_at: string;
  sets: SetResult[];
};
