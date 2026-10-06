import { type FormEvent, useState } from "react";
import { Link } from "react-router";
import { ApiError, logIn, type User } from "./api";

export default function Login({ onLogIn }: { onLogIn: (user: User) => void }) {
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    setBusy(true);
    setError("");
    try {
      onLogIn(await logIn(email, password));
    } catch (err) {
      setError(
        err instanceof ApiError && err.status === 401
          ? "wrong email or password"
          : "could not reach the api",
      );
      setBusy(false);
    }
  };

  return (
    <div className="login-page">

      <h2>University of Calgary - Battle Code</h2>

      <form className="login" onSubmit={submit}>
      <input
        type="email"
        placeholder="Enter email"
        autoComplete="email"
        autoFocus
        value={email}
        onChange={(e) => setEmail(e.target.value)}
      />
      <input
        type="password"
        placeholder="Enter password"
        autoComplete="current-password"
        value={password}
        onChange={(e) => setPassword(e.target.value)}
      />
      <button disabled={busy || !email || !password}>log in</button>
      {error && <p className="error">{error}</p>}
      <p className="dim center">
        <Link to="/register">Create an account</Link> ·{" "}
        <Link to="/docs">Docs</Link> ·{" "}
        <Link to="/leaderboard">Leaderboard</Link>
      </p>
    </form>
    </div>
  );
}
