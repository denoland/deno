// Ported from js-yaml v3.13.1:
// https://github.com/nodeca/js-yaml/commit/665aadda42349dcae869f12040d9b10ef18d12da
// Copyright 2011-2015 by Vitaly Puzrin. All rights reserved. MIT license.
// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

import { Schema } from "../schema.ts";
import { binary, merge, omap, pairs, set, timestamp } from "../type/mod.ts";
import { core } from "./core.ts";

// JS-YAML's default schema for `safeLoad` function.
// It is not described in the YAML specification.
export const def = new Schema({
  explicit: [binary, omap, pairs, set],
  implicit: [timestamp, merge],
  include: [core],
});
