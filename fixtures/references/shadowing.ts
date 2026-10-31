import { login } from "./auth";
export function checkout(login: () => void) {
    login();
    const cb = (login: () => void) => login();
    { const login = () => {}; login(); }
    login();
}
login();
