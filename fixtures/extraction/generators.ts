export function* stream() { yield 1; }
const generator = function* named() { yield 2; };
export async function load<T>(value: T): Promise<T> { return value; }
const typed: (x: number) => number = (x) => x;
