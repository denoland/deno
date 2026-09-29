// The emoji is one extended grapheme cluster whose two scalar values both sit
// outside the Basic Multilingual Plane, so V8 counts four UTF-16 code units
// where the source has two scalar values, and every offset it reports below
// this line names a position two code units right of the one meant. Two lines
// below move with that drift, in opposite directions. `second`'s declaration
// sits below the end of `first`, which is never called, so the range covering
// `first` reaches into it and a function that ran reads as uncovered. The
// never-taken `throw` in `guarded` ends its own line, so its range drifts past
// that line's end and a statement that never ran reads as covered.
export const BADGE = "🕵🏻";

export function first(a: number): number {
  return a + 1;
}
export function second(a: number): number {
  return a + 2;
}

export function guarded(x: unknown): number {
  if (!x) throw new Error("e");
  return 1;
}
