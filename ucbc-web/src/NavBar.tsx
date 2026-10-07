import { useEffect, useState } from "react";
import { Link, NavLink, useLocation } from "react-router";
import { type User } from "./api";
import { buttonClass, cx } from "./ui";

const TITLES: Record<string, string> = {
  "/": "Home",
  "/platform": "Platform",
  "/platform/profile": "Profile",
  "/leaderboard": "Leaderboard",
  "/docs": "Docs",
  "/platform/admin/maps": "Maps",
  "/register": "Sign up",
  "/login": "Log in",
};

const link = ({ isActive }: { isActive: boolean }) =>
  cx(
    "rounded-md px-3 py-1.5 text-sm transition-colors hover:text-fg",
    isActive ? "bg-hover font-medium text-fg" : "text-muted",
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
    const page = pathname.startsWith("/platform/matches/") ? "Match" : TITLES[pathname];
    document.title = page ? `${page} · UCBC` : "UCalgary Battlecode";
    setOpen(false);
  }, [pathname]);

  const links = (
    <>
      <NavLink to="/" end className={link}>
        Home
      </NavLink>
      {user && (
        // The dashboard and its matches; Profile and Maps have their own highlights.
        <NavLink
          to="/platform"
          className={() =>
            link({
              isActive: pathname === "/platform" || pathname.startsWith("/platform/matches/"),
            })
          }
        >
          Platform
        </NavLink>
      )}
      <NavLink to="/leaderboard" className={link}>
        Leaderboard
      </NavLink>
      <NavLink to="/docs" className={link}>
        Docs
      </NavLink>
      {user?.is_admin && (
        <NavLink to="/platform/admin/maps" className={link}>
          Maps
        </NavLink>
      )}
    </>
  );

  return (
    <nav className="sticky top-0 z-20 border-b border-line bg-bg/80 backdrop-blur">
      <div className="mx-auto flex h-14 max-w-6xl items-center gap-6 px-4 sm:px-6">
        <Link to="/" className="flex items-center font-semibold tracking-tight whitespace-nowrap">
          <span>
            UCBC<span className="ml-1.5 hidden font-normal text-muted lg:inline">UCalgary Battlecode</span>
          </span>
        </Link>

        <div className="hidden items-center gap-0.5 sm:flex">{links}</div>

        <div className="ml-auto flex items-center gap-1">
          {user ? (
            <>
              <NavLink
                to="/platform/profile"
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
            !["/login", "/register"].includes(pathname) && (
              <>
                <Link
                  to="/login"
                  className="hidden rounded-lg px-3 py-1.5 text-muted hover:bg-hover hover:text-fg sm:block"
                >
                  Log in
                </Link>
                <Link to="/register" className={cx(buttonClass(), "hidden h-8 min-[360px]:inline-flex")}>
                  Sign up
                </Link>
              </>
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
        <div className="flex flex-col gap-0.5 border-t border-line px-2 py-2 sm:hidden">
          {links}
          {user ? (
            <>
              <NavLink to="/platform/profile" className={link}>
                Profile
              </NavLink>
              <button onClick={onLogOut} className="cursor-pointer rounded-md px-3 py-1.5 text-left text-muted hover:text-fg">
                Log out
              </button>
            </>
          ) : (
            <NavLink to="/login" className={link}>
              Log in
            </NavLink>
          )}
        </div>
      )}
    </nav>
  );
}
