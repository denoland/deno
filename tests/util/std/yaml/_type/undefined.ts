// Ported and adapted from js-yaml-js-types v1.0.0:
// https://github.com/nodeca/js-yaml-js-types/tree/ac537e7bbdd3c2cbbd9882ca3919c520c2dc022b
// Copyright 2011-2015 by Vitaly Puzrin. All rights reserved. MIT license.
// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { Type } from "../type.ts";

export const undefinedType = new Type("tag:yaml.org,2002:js/undefined", {
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
});
