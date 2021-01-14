// Ported from js-yaml v3.13.1:
// https://github.com/nodeca/js-yaml/commit/665aadda42349dcae869f12040d9b10ef18d12da
// Copyright 2011-2015 by Vitaly Puzrin. All rights reserved. MIT license.
// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

import { Schema } from "../schema.ts";
import { map, seq, str } from "../type/mod.ts";

// Standard YAML's Failsafe schema.
// http://www.yaml.org/spec/1.2/spec.html#id2802346
export const failsafe = new Schema({
  explicit: [str, seq, map],
});
