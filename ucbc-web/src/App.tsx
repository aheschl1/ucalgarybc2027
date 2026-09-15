import { useState } from "react";
import type { User } from "./api";
import Login from "./Login";
import Tree from "./Tree";

const KEY = "ucbc-token";

export type Session = { token: string; user: User };

function load(): Session | null {
  const raw = sessionStorage.getItem(KEY);
  return raw ? JSON.parse(raw) : null;
}

export default function App() {
  const [session, setSession] = useState(load);

  const logIn = (s: Session) => {
    sessionStorage.setItem(KEY, JSON.stringify(s));
    setSession(s);
  };
  const logOut = () => {
    sessionStorage.removeItem(KEY);
    setSession(null);
  };

  return session ? <Tree session={session} onLogOut={logOut} /> : <Login onLogIn={logIn} />;
}
