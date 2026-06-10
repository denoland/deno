// This module is intentionally padded with type-only declarations so that the
// transpiled JavaScript would, without line-preserving emit, have far fewer
// lines than the original TypeScript. `targetFunction` therefore ends up on a
// much earlier line in the naively transpiled output than in the source.
//
// See denoland/deno#25349: the Chrome DevTools performance profiler reports the
// raw (un-source-mapped) V8 line numbers, so those line numbers must match the
// original source.

interface ManyFields {
  a: number;
  b: string;
  c: boolean;
  d: number[];
  e: Record<string, number>;
}

type AliasOne = ManyFields | null;
type AliasTwo = AliasOne | undefined;
type AliasThree =
  | AliasTwo
  | { nested: AliasOne; other: AliasTwo };

declare const ambientValue: number;

// Some more type-only padding to widen the gap between the source line and the
// transpiled line.
type Mapped = {
  [K in keyof ManyFields]: ManyFields[K];
};

export function targetFunction(input: number): number {
  let total = 0;
  for (let i = 0; i < input; i++) {
    total += i;
  }
  return total;
}
