// Ported from js-yaml v3.13.1:
// https://github.com/nodeca/js-yaml/commit/665aadda42349dcae869f12040d9b10ef18d12da
// Copyright 2011-2015 by Vitaly Puzrin. All rights reserved. MIT license.
// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import type { SchemaDefinition } from "./schema.ts";
import { DEFAULT_SCHEMA } from "./schema/mod.ts";

export abstract class State {
  constructor(public schema: SchemaDefinition = DEFAULT_SCHEMA) {}
}
