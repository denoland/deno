// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { createExtractor, Parser } from "./create_extractor.ts";
import { Format } from "./_formats.ts";
import { test as _test } from "./test.ts";
import { parse } from "../toml/parse.ts";

export { Format } from "./_formats.ts";

/** @deprecated (will be removed after 0.210.0) Import from {@link https://deno.land/std/front_matter/toml.ts} and use `test(str, ["toml"])` instead. */
export function test(str: string): boolean {
  return _test(str, [Format.TOML]);
}

export const extract = createExtractor({ [Format.TOML]: parse as Parser });
/** @deprecated (will be removed after 0.210.0) Import {@linkcode extract} as a named import instead. */
export default extract;
