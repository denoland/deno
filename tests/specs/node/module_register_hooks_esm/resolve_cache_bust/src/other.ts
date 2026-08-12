// deno-lint-ignore no-explicit-any
const g = globalThis as any;
g.evalCount = (g.evalCount ?? 0) + 1;

export const other: string = `Value${g.evalCount}`;
