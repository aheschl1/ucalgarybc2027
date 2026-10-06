import { type FormEvent, useState } from "react";
import { Link } from "react-router";
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
    <AuthCard
      footer={
        <Link to="/" className="hover:text-fg hover:underline">
          Log in
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
        <Field label="Display name">
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
