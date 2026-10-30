export function outer() {
  function inner() { return 1; }
  const callback = () => inner();
  return callback();
}
const bound = () => { return 1; };
register(() => outer());
