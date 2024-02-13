// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { createExtractor, Parser } from "./create_extractor.ts";
import { test as _test } from "./test.ts";

export { Format } from "./_formats.ts";

/** @deprecated (will be removed after 0.210.0) Import from {@link https://deno.land/std/front_matter/json.ts} and use `test(str, ["json"])` instead. */
export function test(str: string): boolean {
  return _test(str, ["json"]);
}

export const extract = createExtractor({ json: JSON.parse as Parser });
/** @deprecated (will be removed after 0.210.0) Import {@linkcode extract} as a named import instead. */
export default extract;
