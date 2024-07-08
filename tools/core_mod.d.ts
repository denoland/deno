/// <reference path="../../deno_core/core/internal.d.ts" />

// deno-lint-ignore no-explicit-any
export const core: any;

// deno-lint-ignore no-explicit-any
type UncurryThis<T extends (this: any, ...args: any[]) => any> = (
  self: ThisParameterType<T>,
  ...args: Parameters<T>
) => ReturnType<T>;

export const primordials: typeof __bootstrap.primordials;

// deno-lint-ignore no-explicit-any
export const internals: any;
