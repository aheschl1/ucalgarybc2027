import { type FormEvent, type ReactNode, useState } from "react";
import { Link, useLocation } from "react-router";
import { ApiError, logIn, type User } from "./api";
import { Button, ErrorText, Field, Input } from "./ui";

export default function Login({ onLogIn }: { onLogIn: (user: User) => void }) {
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  // Carries `next` across, so signing up instead still lands where you were going.
  const { search } = useLocation();

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
      title="Welcome back"
      subtitle="Log in to submit bots and queue matches."
      footer={
        <>
          No account?{" "}
          <Link to={`/register${search}`} className="font-medium text-fg hover:underline">
            Sign up
          </Link>
        </>
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
export function AuthCard({
  title,
  subtitle,
  footer,
  children,
}: {
  title: string;
  subtitle: string;
  footer: ReactNode;
  children: ReactNode;
}) {
  return (
    <main className="relative flex min-h-[calc(100dvh-3.5rem)] flex-col items-center justify-center overflow-hidden px-4 py-12">
      <div className="grid-bg pointer-events-none absolute inset-0" />
      <div className="relative w-full max-w-sm animate-rise">
        <div className="rounded-xl border border-line bg-bg p-6 shadow-xl shadow-fg/5">
          <h1 className="text-lg font-semibold tracking-tight">{title}</h1>
          <p className="mt-1 mb-6 text-muted">{subtitle}</p>
          {children}
        </div>
        <p className="mt-6 text-center text-muted">{footer}</p>
      </div>
    </main>
  );
}
