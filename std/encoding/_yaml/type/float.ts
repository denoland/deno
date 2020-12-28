// Ported from js-yaml v3.13.1:
// https://github.com/nodeca/js-yaml/commit/665aadda42349dcae869f12040d9b10ef18d12da
// Copyright 2011-2015 by Vitaly Puzrin. All rights reserved. MIT license.
// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

import { StyleVariant, Type } from "../type.ts";
import { Any, isNegativeZero } from "../utils.ts";

const YAML_FLOAT_PATTERN = new RegExp(
  // 2.5e4, 2.5 and integers
  "^(?:[-+]?(?:0|[1-9][0-9_]*)(?:\\.[0-9_]*)?(?:[eE][-+]?[0-9]+)?" +
    // .2e4, .2
    // special case, seems not from spec
    "|\\.[0-9_]+(?:[eE][-+]?[0-9]+)?" +
    // 20:59
    "|[-+]?[0-9][0-9_]*(?::[0-5]?[0-9])+\\.[0-9_]*" +
    // .inf
    "|[-+]?\\.(?:inf|Inf|INF)" +
    // .nan
    "|\\.(?:nan|NaN|NAN))$",
);

function resolveYamlFloat(data: string): boolean {
  if (
    !YAML_FLOAT_PATTERN.test(data) ||
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
  const digits: number[] = [];

  if ("+-".indexOf(value[0]) >= 0) {
    value = value.slice(1);
  }

  if (value === ".inf") {
    return sign === 1 ? Number.POSITIVE_INFINITY : Number.NEGATIVE_INFINITY;
  }
  if (value === ".nan") {
    return NaN;
  }
  if (value.indexOf(":") >= 0) {
    value.split(":").forEach((v): void => {
      digits.unshift(parseFloat(v));
    });

    let valueNb = 0.0;
    let base = 1;

    digits.forEach((d): void => {
      valueNb += d * base;
      base *= 60;
    });

    return sign * valueNb;
  }
  return sign * parseFloat(value);
}

const SCIENTIFIC_WITHOUT_DOT = /^[-+]?[0-9]+e/;

function representYamlFloat(object: Any, style?: StyleVariant): Any {
  if (isNaN(object)) {
    switch (style) {
      case "lowercase":
        return ".nan";
      case "uppercase":
        return ".NAN";
      case "camelcase":
        return ".NaN";
    }
  } else if (Number.POSITIVE_INFINITY === object) {
    switch (style) {
      case "lowercase":
        return ".inf";
      case "uppercase":
        return ".INF";
      case "camelcase":
        return ".Inf";
    }
  } else if (Number.NEGATIVE_INFINITY === object) {
    switch (style) {
      case "lowercase":
        return "-.inf";
      case "uppercase":
        return "-.INF";
      case "camelcase":
        return "-.Inf";
    }
  } else if (isNegativeZero(object)) {
    return "-0.0";
  }

  const res = object.toString(10);

  // JS stringifier can build scientific format without dots: 5e-100,
  // while YAML requres dot: 5.e-100. Fix it with simple hack

  return SCIENTIFIC_WITHOUT_DOT.test(res) ? res.replace("e", ".e") : res;
}

function isFloat(object: Any): boolean {
  return (
    Object.prototype.toString.call(object) === "[object Number]" &&
    (object % 1 !== 0 || isNegativeZero(object))
  );
}

export const float = new Type("tag:yaml.org,2002:float", {
  construct: constructYamlFloat,
  defaultStyle: "lowercase",
  kind: "scalar",
  predicate: isFloat,
  represent: representYamlFloat,
  resolve: resolveYamlFloat,
});
