import { type FormEvent, useState } from "react";
import { Link, useLocation } from "react-router";
import { describe, logIn, signUp, type User } from "./api";
import { AuthCard } from "./Login";
import { Button, ErrorText, Field, Input } from "./ui";

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
  const { search } = useLocation();

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    setBusy(true);
    setError("");
    try {
      // Signing up does not log you in, so the app does it straight after. The
      // session swaps this route for a redirect: to `next`, else the profile.
      await signUp(email, displayName, password);
      onLogIn(await logIn(email, password));
    } catch (err) {
      setError(describe(err));
      setBusy(false);
    }
  };

  return (
    <AuthCard
      title="Create an account"
      subtitle="You start on a team of your own; invite others later."
      footer={
        <>
          Have an account?{" "}
          <Link to={`/login${search}`} className="font-medium text-fg hover:underline">
            Log in
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
        <Field label="Display name, shown to other teams">
          <Input
            autoComplete="nickname"
            maxLength={64}
            value={displayName}
            onChange={(e) => setDisplayName(e.target.value)}
          />
        </Field>
        <Field label="Password">
          <Input
            type="password"
            placeholder="8+ characters"
            autoComplete="new-password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
          />
        </Field>
        {error && <ErrorText>{error}</ErrorText>}
        <Button
          className="mt-1 w-full"
          disabled={busy || !email || !displayName || password.length < 8}
        >
          Sign up
        </Button>
      </form>
    </AuthCard>
  );
}
