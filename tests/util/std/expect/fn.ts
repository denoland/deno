// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright 2019 Allain Lalonde. All rights reserved. ISC License.
// deno-lint-ignore-file no-explicit-any ban-types

import { MOCK_SYMBOL, MockCall } from "./_mock_util.ts";

export function fn(...stubs: Function[]) {
  const calls: MockCall[] = [];

  const f = (...args: any[]) => {
    const stub = stubs.length === 1
      // keep reusing the first
      ? stubs[0]
      // pick the exact mock for the current call
      : stubs[calls.length];

    try {
      const returned = stub ? stub(...args) : undefined;
      calls.push({
        args,
        returned,
        timestamp: Date.now(),
        returns: true,
        throws: false,
      });
      return returned;
    } catch (err) {
      calls.push({
        args,
        timestamp: Date.now(),
        returns: false,
        thrown: err,
        throws: true,
      });
      throw err;
    }
  };

  Object.defineProperty(f, MOCK_SYMBOL, {
    value: { calls },
    writable: false,
  });

  return f;
}
