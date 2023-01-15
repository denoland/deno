export function test() {
  return 42;
}
if (import.meta.main) {
  test();
}
