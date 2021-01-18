// Ported from js-yaml v3.13.1:
// https://github.com/nodeca/js-yaml/commit/665aadda42349dcae869f12040d9b10ef18d12da
// Copyright 2011-2015 by Vitaly Puzrin. All rights reserved. MIT license.
// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

import { Schema } from "../schema.ts";
import { json } from "./json.ts";

// Standard YAML's Core schema.
// http://www.yaml.org/spec/1.2/spec.html#id2804923
export const core = new Schema({
  include: [json],
});
