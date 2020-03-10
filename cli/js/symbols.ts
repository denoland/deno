// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { internalSymbol } from "./internals.ts";
import { customInspect } from "./web/console.ts";

/** Special Deno related symbols. */
export const symbols = {
  /** Symbol to access exposed internal Deno API */
  internal: internalSymbol,
  /** A symbol which can be used as a key for a custom method which will be called
   * when `Deno.inspect()` is called, or when the object is logged to the console.
   */
  customInspect
};
