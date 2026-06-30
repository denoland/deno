// Ported and adapted from js-yaml-js-types v1.0.0:
// https://github.com/nodeca/js-yaml-js-types/tree/ac537e7bbdd3c2cbbd9882ca3919c520c2dc022b
// Copyright 2011-2015 by Vitaly Puzrin. All rights reserved. MIT license.
// Copyright 2018-2026 the Deno authors. MIT license.

import type { Type } from "../_type.ts";

export const undefinedType: Type<"scalar", undefined> = {
  tag: "tag:yaml.org,2002:js/undefined",
  kind: "scalar",
  resolve() {
    return true;
  },
  construct() {
    return undefined;
  },
  predicate(object) {
    return typeof object === "undefined";
  },
  represent() {
    return "";
  },
};
