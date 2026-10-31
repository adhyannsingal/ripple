import run, { login as authenticate, type Session } from "./auth";
import type Config from "./types";
import "./setup";
export { authenticate as login };
export { default as start, login as verify, type Session } from "./auth";
export * from "./barrel";
export function checkout(input: Session) {
    const copy = authenticate;
    run(copy, input);
    authenticate(input);
}
