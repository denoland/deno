// Ported and adapted from js-yaml-js-types v1.0.0:
// https://github.com/nodeca/js-yaml-js-types/tree/ac537e7bbdd3c2cbbd9882ca3919c520c2dc022b
// Copyright 2011-2015 by Vitaly Puzrin. All rights reserved. MIT license.
// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { Type } from "../type.ts";
import type { Any } from "../_utils.ts";

// Note: original implementation used Esprima to handle functions
// To avoid dependencies, we'll just try to check if we can construct a function from given string
function reconstructFunction(code: string) {
  const func = new Function(`return ${code}`)();
  if (!(func instanceof Function)) {
    throw new TypeError(`Expected function but got ${typeof func}: ${code}`);
  }
  return func;
}

export const func = new Type("tag:yaml.org,2002:js/function", {
  kind: "scalar",
  resolve(data: Any) {
    if (data === null) {
      return false;
    }
    try {
      reconstructFunction(`${data}`);
      return true;
    } catch (_err) {
      return false;
    }
  },
  construct(data: string) {
    return reconstructFunction(data);
  },
  predicate(object: unknown) {
    return object instanceof Function;
  },
  represent(object: (...args: Any[]) => Any) {
    return object.toString();
  },
});
