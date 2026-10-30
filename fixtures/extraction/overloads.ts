export function convert(value: number): number;
export function convert(value: string): string;
export function convert(value: number | string) { return value; }
export class Box {
  constructor(value: number);
  constructor(value: number) {}
  compute(value: number): number;
  compute(value: string): string;
  compute(value: number | string) { return value; }
  async map<T>(value: T): Promise<T> { return value; }
}
