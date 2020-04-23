// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { internalSymbol } from "./internals.ts";
import { customInspect } from "./web/console.ts";
import { cookiesIteratorSymbol } from "./web/headers.ts";

export const symbols = {
  internal: internalSymbol,
  customInspect,
  cookiesIterator: cookiesIteratorSymbol,
};
