import { useEffect, useState } from "react";
import { Navigate, Route, Routes } from "react-router";
import { get, logOut, makeApi, type User } from "./api";
import Login from "./Login";
import Profile from "./Profile";
import Register from "./Register";
import Tree from "./Tree";
import Viewer from "./Viewer";

export default function App() {
  // undefined until the API says whether the session cookie is live.
  const [user, setUser] = useState<User | null | undefined>(undefined);

  useEffect(() => {
    get<User>("/users/me").then(setUser, () => setUser(null));
  }, []);

  const leave = () => {
    setUser(null);
    logOut().catch(() => {});
  };

  if (user === undefined) return null;
  if (!user) {
    // Signing in at any address lands on it, since the route table below then applies.
    return (
      <Routes>
        <Route path="/register" element={<Register onLogIn={setUser} />} />
        <Route path="*" element={<Login onLogIn={setUser} />} />
      </Routes>
    );
  }

  const api = makeApi(leave);
  return (
    <Routes>
      <Route
        path="/"
        element={<Tree user={user} api={api} onLogOut={leave} />}
      />
      <Route path="/profile" element={<Profile user={user} api={api} />} />
      <Route path="/viewer/:matchId" element={<Viewer api={api} />} />
      {/* Where signing up lands: the session arrives while the browser is still here. */}
      <Route path="/register" element={<Navigate to="/profile" replace />} />
      <Route path="*" element={<Navigate to="/" replace />} />
    </Routes>
  );
}
