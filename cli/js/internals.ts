// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
export const internalSymbol = Symbol("Deno.internal");

// The object where all the internal fields for testing will be living.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export const internalObject: { [key: string]: any } = {};

// Register a field to internalObject for test access,
// through Deno[Deno.symbols.internal][name].
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function exposeForTest(name: string, value: any): void {
  Object.defineProperty(internalObject, name, {
    value,
    enumerable: false,
  });
}
