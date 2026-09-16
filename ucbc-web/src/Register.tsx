import { type FormEvent, useState } from "react";
import { Link } from "react-router";
import { describe, logIn, signUp, type User } from "./api";

export default function Register({
  onLogIn,
}: {
  onLogIn: (user: User) => void;
}) {
  const [email, setEmail] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    setBusy(true);
    setError("");
    try {
      // Signing up does not log you in, so the app does it straight after. The
      // session swaps the route table, which is what lands this on the profile.
      await signUp(email, displayName, password);
      onLogIn(await logIn(email, password));
    } catch (err) {
      setError(describe(err));
      setBusy(false);
    }
  };

  return (
    <form className="login" onSubmit={submit}>
      <input
        type="email"
        placeholder="email"
        autoComplete="email"
        autoFocus
        value={email}
        onChange={(e) => setEmail(e.target.value)}
      />
      <input
        placeholder="display name"
        autoComplete="nickname"
        maxLength={64}
        value={displayName}
        onChange={(e) => setDisplayName(e.target.value)}
      />
      <input
        type="password"
        placeholder="password (8 or more)"
        autoComplete="new-password"
        value={password}
        onChange={(e) => setPassword(e.target.value)}
      />
      <button disabled={busy || !email || !displayName || password.length < 8}>
        sign up
      </button>
      {error && <p className="error">{error}</p>}
      <p className="dim center">
        <Link to="/">already have an account?</Link>
      </p>
    </form>
  );
}
