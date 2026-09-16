import { type FormEvent, useState } from "react";
import { ApiError, logIn, type User } from "./api";

export default function Login({ onLogIn }: { onLogIn: (user: User) => void }) {
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    setBusy(true);
    setError("");
    try {
      onLogIn(await logIn(username, password));
    } catch (err) {
      setError(
        err instanceof ApiError && err.status === 401
          ? "wrong username or password"
          : "could not reach the api",
      );
      setBusy(false);
    }
  };

  return (
    <form className="login" onSubmit={submit}>
      <input
        placeholder="username"
        autoComplete="username"
        autoFocus
        value={username}
        onChange={(e) => setUsername(e.target.value)}
      />
      <input
        type="password"
        placeholder="password"
        autoComplete="current-password"
        value={password}
        onChange={(e) => setPassword(e.target.value)}
      />
      <button disabled={busy || !username || !password}>log in</button>
      {error && <p className="error">{error}</p>}
    </form>
  );
}
