// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { createExtractor, Parser } from "./create_extractor.ts";
import { parse as parseYAML } from "../yaml/parse.ts";
import { parse as parseTOML } from "../toml/parse.ts";

export { Format } from "./_formats.ts";
/** @deprecated (will be removed after 0.210.0) Import from {@link https://deno.land/std/front_matter/test.ts} instead. */
export { test } from "./test.ts";

export const extract = createExtractor({
  yaml: parseYAML as Parser,
  toml: parseTOML as Parser,
  json: JSON.parse as Parser,
});
/** @deprecated (will be removed after 0.210.0) Import {@linkcode extract} as a named import instead. */
export default extract;
