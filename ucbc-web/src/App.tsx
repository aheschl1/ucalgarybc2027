import { useEffect, useState } from "react";
import { get, logOut, type User } from "./api";
import Login from "./Login";
import Tree from "./Tree";

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
  return user ? <Tree user={user} onLogOut={leave} /> : <Login onLogIn={setUser} />;
}
