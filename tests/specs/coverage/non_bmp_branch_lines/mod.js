// The emoji is two scalar values outside the Basic Multilingual Plane, so every
// offset V8 reports below it is two UTF-16 code units ahead of the scalar-value
// offset naming the same position. The `{` opening the guard is the last
// position on its line, so reading the range that starts there as a
// scalar-value offset lands two positions further on, past the newline, and
// files the branch under the line below the one it is on.
export const BADGE = "🕵🏻";
export function guarded(x) {
  if (x) {
    return 1;
  }
  return 2;
}
