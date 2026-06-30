// Ported from js-yaml v3.13.1:
// https://github.com/nodeca/js-yaml/commit/665aadda42349dcae869f12040d9b10ef18d12da
// Copyright 2011-2015 by Vitaly Puzrin. All rights reserved. MIT license.
// Copyright 2018-2026 the Deno authors. MIT license.

import type { StyleVariant, Type } from "../_type.ts";
import { isNegativeZero } from "../_utils.ts";

const YAML_FLOAT_REGEXP = new RegExp(
  // 2.5e4, 2.5 and integers
  "^(?:[-+]?(?:0|[1-9][0-9_]*)(?:\\.[0-9_]*)?(?:[eE][-+]?[0-9]+)?" +
    // .2e4, .2
    // special case, seems not from spec
    "|\\.[0-9_]+(?:[eE][-+]?[0-9]+)?" +
    // .inf
    "|[-+]?\\.(?:inf|Inf|INF)" +
    // .nan
    "|\\.(?:nan|NaN|NAN))$",
);

function resolveYamlFloat(data: string): boolean {
  if (
    !YAML_FLOAT_REGEXP.test(data) ||
    // Quick hack to not allow integers end with `_`
    // Probably should update regexp & check speed
    data[data.length - 1] === "_"
  ) {
    return false;
  }

  return true;
}

function constructYamlFloat(data: string): number {
  let value = data.replace(/_/g, "").toLowerCase();
  const sign = value[0] === "-" ? -1 : 1;

  if (value[0] && "+-".includes(value[0])) {
    value = value.slice(1);
  }

  if (value === ".inf") {
    return sign === 1 ? Number.POSITIVE_INFINITY : Number.NEGATIVE_INFINITY;
  }
  if (value === ".nan") {
    return NaN;
  }
  return sign * parseFloat(value);
}

const SCIENTIFIC_WITHOUT_DOT_REGEXP = /^[-+]?[0-9]+e/;

function representYamlFloat(
  // deno-lint-ignore ban-types
  object: number | Number,
  style?: StyleVariant,
): string {
  const value = object instanceof Number ? object.valueOf() : object;
  if (isNaN(value)) {
    switch (style) {
      case "lowercase":
        return ".nan";
      case "uppercase":
        return ".NAN";
      case "camelcase":
        return ".NaN";
    }
  } else if (Number.POSITIVE_INFINITY === value) {
    switch (style) {
      case "lowercase":
        return ".inf";
      case "uppercase":
        return ".INF";
      case "camelcase":
        return ".Inf";
    }
  } else if (Number.NEGATIVE_INFINITY === value) {
    switch (style) {
      case "lowercase":
        return "-.inf";
      case "uppercase":
        return "-.INF";
      case "camelcase":
        return "-.Inf";
    }
  } else if (isNegativeZero(value)) {
    return "-0.0";
  }

  const res = value.toString(10);

  // JS stringifier can build scientific format without dots: 5e-100,
  // while YAML requires dot: 5.e-100. Fix it with simple hack

  return SCIENTIFIC_WITHOUT_DOT_REGEXP.test(res) ? res.replace("e", ".e") : res;
}

function isFloat(object: unknown): object is number {
  if (object instanceof Number) object = object.valueOf();
  return typeof object === "number" &&
    (object % 1 !== 0 || isNegativeZero(object));
}

export const float: Type<"scalar", number> = {
  tag: "tag:yaml.org,2002:float",
  construct: constructYamlFloat,
  defaultStyle: "lowercase",
  kind: "scalar",
  predicate: isFloat,
  represent: representYamlFloat,
  resolve: resolveYamlFloat,
};
