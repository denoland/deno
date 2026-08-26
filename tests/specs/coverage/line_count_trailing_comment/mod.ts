// The two functions contain identical executable code. In each, the guard's
// branch is never taken (so the guard line is uncovered) and the return runs
// (so the return line is covered). withComment puts a trailing comment on both
// of those lines. A trailing comment must not change the reported hit count of
// the line it sits on, in either direction: it must not un-zero the uncovered
// guard, and it must not zero the covered return.
export function noComment(x: unknown): number {
  if (!x) throw new Error("e");
  return 1;
}
export function withComment(x: unknown): number {
  if (!x) throw new Error("e"); // a trailing comment
  return 1; // a trailing comment
}
