export function test(a, b) {
  if (a) {
    return 0;
  }
  // Not covered
  if (b) {
    return 0;
  }
  return 1;
}
