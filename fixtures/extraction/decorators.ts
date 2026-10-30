@sealed
class Decorated {
  field = () => 1;
  [key]() { return 1; }
  run() { return 2; }
}
