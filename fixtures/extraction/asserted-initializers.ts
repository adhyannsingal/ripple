function known() { return 1; }
export const asserted = (() => 1) as Fn;
const checked = (() => 2) satisfies Fn;
const angle = <Fn>(() => 3);
const definite = (() => 4)!;
export const Wrapped = (class { run() {} }) as Constructor;
