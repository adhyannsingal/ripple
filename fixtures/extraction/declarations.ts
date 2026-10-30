export function identity<T>(value: T): T { return value; }
export const arrow = (value: number): number => value + 1;
const expression = function named(value: number) { return value; };
export { expression as alias };
class First {
  public run() { return 1; }
  static run() { return 2; }
  get value() { return 1; }
  set value(value: number) {}
  private hidden() {}
  protected guarded() {}
  #secret() {}
  "a.b"() {}
}
export { First as Api };
class Second { run() {} }
