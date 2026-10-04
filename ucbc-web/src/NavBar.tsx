import { Link, useLocation } from "react-router";
import { type User } from "./api";

export default function NavBar({
    onLogOut,
    user,
}: {
    onLogOut: () => void;
    user: User;
}) {
    const pageLocation = useLocation();
    let pageName = "U of C - Battle Code";
    if(pageLocation.pathname === "/") {
        pageName += " | Home";
    }
    else if(pageLocation.pathname === "/profile") {
        pageName += "  | Profile";
    }
    else if(pageLocation.pathname === "/docs") {
        pageName += " | Docs";
    }
    else if(pageLocation.pathname === "/leaderboard") {
        pageName += " | Leaderboard";
    }

    return (
        <nav className="navbar">
            <div className="navbar-left">
                <h3>{pageName}</h3>
            </div>

            <div className="navbar-right">
                <Link to="/">Home</Link>
                <Link to="/profile">{user.display_name}</Link>
                <Link to="/leaderboard">Leaderboard</Link>
                <Link to="/docs">Docs</Link>
                <button onClick={onLogOut}>Logout</button>
            </div>
        </nav>
    )
}
