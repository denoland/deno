// Ported from js-yaml v3.13.1:
// https://github.com/nodeca/js-yaml/commit/665aadda42349dcae869f12040d9b10ef18d12da
// Copyright 2011-2015 by Vitaly Puzrin. All rights reserved. MIT license.
// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

export type { ParseOptions } from "./_yaml/parse.ts";
export { parse, parseAll } from "./_yaml/parse.ts";
export type { DumpOptions as StringifyOptions } from "./_yaml/stringify.ts";
export { stringify } from "./_yaml/stringify.ts";
export type { SchemaDefinition } from "./_yaml/schema.ts";
export type { StyleVariant } from "./_yaml/type.ts";
export {
  CORE_SCHEMA,
  DEFAULT_SCHEMA,
  FAILSAFE_SCHEMA,
  JSON_SCHEMA,
} from "./_yaml/schema/mod.ts";
