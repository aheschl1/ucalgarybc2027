import { useEffect, useState } from "react";
import { Link, NavLink, useLocation } from "react-router";
import { type User } from "./api";
import { cx } from "./ui";

const TITLES: Record<string, string> = {
  "/": "Home",
  "/profile": "Profile",
  "/leaderboard": "Leaderboard",
  "/docs": "Docs",
  "/register": "Sign up",
};

const link = ({ isActive }: { isActive: boolean }) =>
  cx(
    "relative py-4 text-sm transition-colors hover:text-fg",
    isActive
      ? "font-medium text-fg after:absolute after:inset-x-0 after:-bottom-px after:h-0.5 after:bg-accent"
      : "text-muted",
  );

/** The bar on every page. Without a user it offers logging in instead of the account. */
export default function NavBar({
  user,
  onLogOut,
}: {
  user?: User;
  onLogOut?: () => void;
}) {
  const { pathname } = useLocation();
  const [open, setOpen] = useState(false);

  useEffect(() => {
    const page = pathname.startsWith("/viewer/") ? "Match" : TITLES[pathname];
    document.title = page ? `${page} · UCBC` : "UCBC";
    setOpen(false);
  }, [pathname]);

  const links = (
    <>
      {user && (
        <NavLink to="/" end className={link}>
          Home
        </NavLink>
      )}
      <NavLink to="/leaderboard" className={link}>
        Leaderboard
      </NavLink>
      <NavLink to="/docs" className={link}>
        Docs
      </NavLink>
    </>
  );

  return (
    <nav className="sticky top-0 z-20 border-b border-line bg-bg/80 backdrop-blur">
      <div className="mx-auto flex h-14 max-w-6xl items-center gap-8 px-4 sm:px-6">
        <Link to="/" className="font-semibold tracking-tight">
          UCBC
        </Link>

        <div className="hidden items-center gap-6 sm:flex">{links}</div>

        <div className="ml-auto flex items-center gap-1">
          {user ? (
            <>
              <NavLink
                to="/profile"
                className={({ isActive }) =>
                  cx(
                    "flex items-center gap-2 rounded-lg px-2 py-1.5 hover:bg-hover",
                    isActive && "bg-hover",
                  )
                }
              >
                <span className="grid size-6 place-items-center rounded-full bg-fg text-[11px] font-semibold text-bg uppercase">
                  {user.display_name.slice(0, 1)}
                </span>
                <span className="hidden max-w-40 truncate sm:inline">{user.display_name}</span>
              </NavLink>
              <button
                onClick={onLogOut}
                className="hidden cursor-pointer rounded-lg px-2.5 py-1.5 text-muted hover:bg-hover hover:text-fg sm:block"
              >
                Log out
              </button>
            </>
          ) : (
            pathname !== "/" && (
              <Link
                to="/"
                className="rounded-lg bg-fg px-3 py-1.5 font-medium text-bg hover:bg-fg/85"
              >
                Log in
              </Link>
            )
          )}
          <button
            className="cursor-pointer rounded-lg p-2 text-muted hover:bg-hover hover:text-fg sm:hidden"
            aria-label="Menu"
            aria-expanded={open}
            onClick={() => setOpen(!open)}
          >
            <svg viewBox="0 0 20 20" className="size-5" fill="none" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round">
              {open ? <path d="M5 5l10 10M15 5L5 15" /> : <path d="M3 6h14M3 10h14M3 14h14" />}
            </svg>
          </button>
        </div>
      </div>

      {open && (
        <div className="flex flex-col border-t border-line px-4 pb-2 sm:hidden [&>a]:py-2.5 [&>a]:after:hidden">
          {links}
          {user && (
            <button onClick={onLogOut} className="cursor-pointer py-2.5 text-left text-muted hover:text-fg">
              Log out
            </button>
          )}
        </div>
      )}
    </nav>
  );
}
