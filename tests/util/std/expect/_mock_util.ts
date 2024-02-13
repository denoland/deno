// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// deno-lint-ignore-file no-explicit-any

export const MOCK_SYMBOL = Symbol.for("@MOCK");

export type MockCall = {
  args: any[];
  returned?: any;
  thrown?: any;
  timestamp: number;
  returns: boolean;
  throws: boolean;
};

export function getMockCalls(f: any): MockCall[] {
  const mockInfo = f[MOCK_SYMBOL];
  if (!mockInfo) {
    throw new Error("Received function must be a mock or spy function");
  }

  return [...mockInfo.calls];
}
