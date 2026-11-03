import { alias as use } from './barrel';
// non-ASCII before a reference: 😀 é
export function caller() { return use('😀') + use('é'); }
export function reference() { return use; }
export function shadow(use: () => string) { return use(); }
