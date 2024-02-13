// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { expect } from "./expect.ts";
import { fn } from "./fn.ts";

Deno.test("expect().toHaveBeenCalled()", () => {
  const mockFn = fn();
  mockFn();
  expect(mockFn).toHaveBeenCalled();
});
