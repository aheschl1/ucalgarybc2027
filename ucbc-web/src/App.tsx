import { useEffect, useState } from "react";
import { Navigate, Outlet, Route, Routes, useLocation, useNavigate, useSearchParams } from "react-router";
import { get, logOut, makeApi, type User } from "./api";
import Docs from "./Docs";
import Home from "./Home";
import Leaderboard from "./Leaderboard";
import Login from "./Login";
import Maps from "./Maps";
import NavBar from "./NavBar";
import Profile from "./Profile";
import Register from "./Register";
import Tree from "./Tree";
import Viewer from "./Viewer";

export default function App() {
  // undefined until the API says whether the session cookie is live.
  const [user, setUser] = useState<User | null | undefined>(undefined);
  const navigate = useNavigate();
  const [params] = useSearchParams();
  // Only a path on this site, so a crafted link cannot bounce you elsewhere after logging in.
  const next = params.get("next");
  const safe = next?.startsWith("/") && !next.startsWith("//") ? next : undefined;

  useEffect(() => {
    get<User>("/users/me").then(setUser, () => setUser(null));
  }, []);

  // The session ending (logging out, or a 401) leaves /platform/* to send you to /login.
  const leave = () => {
    setUser(null);
    logOut().catch(() => {});
  };

  if (user === undefined) return null;
  const api = user && makeApi(leave);

  return (
    <Routes>
      <Route
        element={
          <>
            <NavBar
              user={user ?? undefined}
              onLogOut={() => {
                leave();
                navigate("/");
              }}
            />
            <Outlet />
          </>
        }
      >
        <Route index element={<Home user={user ?? undefined} />} />
        <Route path="docs" element={<Docs />} />
        <Route path="leaderboard" element={<Leaderboard teamId={user?.team_id} />} />
        {/* Logging in swaps these for redirects, which is what lands you on `next`. */}
        <Route
          path="login"
          element={
            user ? (
              <Navigate to={safe ?? "/platform"} replace />
            ) : (
              <Login onLogIn={setUser} />
            )
          }
        />
        <Route
          path="register"
          element={
            // A new account lands on the profile to pick a team, unless it was headed somewhere.
            user ? (
              <Navigate to={safe ?? "/platform/profile"} replace />
            ) : (
              <Register onLogIn={setUser} />
            )
          }
        />

        {user && api ? (
          <Route path="platform">
            <Route index element={<Tree user={user} api={api} />} />
            <Route
              path="profile"
              element={
                <Profile
                  user={user}
                  api={api}
                  onMove={(team) => setUser({ ...user, team_id: team.id })}
                />
              }
            />
            <Route path="matches/:matchId" element={<Viewer api={api} />} />
            {user.is_admin && <Route path="admin/maps" element={<Maps api={api} />} />}
            <Route path="*" element={<Navigate to="/platform" replace />} />
          </Route>
        ) : (
          <Route path="platform/*" element={<ToLogin />} />
        )}
        <Route path="*" element={<Navigate to="/" replace />} />
      </Route>
    </Routes>
  );
}

/** Sends a visitor without a session to log in, then back to where they were going. */
function ToLogin() {
  const { pathname, search } = useLocation();
  return <Navigate to={`/login?next=${encodeURIComponent(pathname + search)}`} replace />;
}
