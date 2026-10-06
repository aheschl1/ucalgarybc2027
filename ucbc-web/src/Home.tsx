import { Link } from "react-router";
import { type User } from "./api";
import { Page, TextLink } from "./ui";

/** The landing page, public. */
export default function Home({ user }: { user?: User }) {
  return (
    <Page>
      <h1 className="text-2xl font-semibold tracking-tight">UCalgary Battlecode</h1>
      <p className="mt-3 text-muted">A 1v1 game competition between bots written in Python.</p>
      <div className="mt-6 flex flex-wrap items-center gap-4">
        <Link
          to={user ? "/platform" : "/register"}
          className="inline-flex h-9 items-center rounded-lg bg-fg px-3.5 text-sm font-medium text-bg hover:bg-fg/85"
        >
          {user ? "Open platform" : "Sign up"}
        </Link>
        {!user && <TextLink to="/login">Log in</TextLink>}
        <TextLink to="/docs">Read the docs</TextLink>
      </div>
    </Page>
  );
}
