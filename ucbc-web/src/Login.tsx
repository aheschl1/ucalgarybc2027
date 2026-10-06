import { type FormEvent, type ReactNode, useState } from "react";
import { Link } from "react-router";
import { ApiError, logIn, type User } from "./api";
import { Button, ErrorText, Field, Input } from "./ui";

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
          ? "Wrong email or password"
          : "Could not reach the API",
      );
      setBusy(false);
    }
  };

  return (
    <AuthCard
      footer={
        <Link to="/register" className="hover:text-fg hover:underline">
          Create an account
        </Link>
      }
    >
      <form className="flex flex-col gap-4" onSubmit={submit}>
        <Field label="Email">
          <Input
            type="email"
            autoComplete="email"
            autoFocus
            value={email}
            onChange={(e) => setEmail(e.target.value)}
          />
        </Field>
        <Field label="Password">
          <Input
            type="password"
            autoComplete="current-password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
          />
        </Field>
        {error && <ErrorText>{error}</ErrorText>}
        <Button className="mt-1 w-full" disabled={busy || !email || !password}>
          Log in
        </Button>
      </form>
    </AuthCard>
  );
}

/** Logging in and signing up: one narrow card in the middle of the page. */
export function AuthCard({ footer, children }: { footer: ReactNode; children: ReactNode }) {
  return (
    <main className="flex min-h-[calc(100dvh-3.5rem)] flex-col items-center justify-center px-4 py-12">
      <div className="w-full max-w-sm">
        <div className="rounded-xl border border-line p-6 shadow-sm">{children}</div>
        <p className="mt-6 text-center text-muted">{footer}</p>
      </div>
    </main>
  );
}
