// Ported from js-yaml v3.13.1:
// https://github.com/nodeca/js-yaml/commit/665aadda42349dcae869f12040d9b10ef18d12da
// Copyright 2011-2015 by Vitaly Puzrin. All rights reserved. MIT license.
// Copyright 2018-2026 the Deno authors. MIT license.

import type { Type } from "../_type.ts";
import { isNegativeZero } from "../_utils.ts";

function isCharCodeInRange(c: number, lower: number, upper: number): boolean {
  return lower <= c && c <= upper;
}

function isHexCode(c: number): boolean {
  return (
    isCharCodeInRange(c, 0x30, 0x39) || // 0-9
    isCharCodeInRange(c, 0x41, 0x46) || // A-F
    isCharCodeInRange(c, 0x61, 0x66) // a-f
  );
}

function isOctCode(c: number): boolean {
  return isCharCodeInRange(c, 0x30, 0x37); // 0-7
}

function isDecCode(c: number): boolean {
  return isCharCodeInRange(c, 0x30, 0x39); // 0-9
}

function resolveYamlInteger(data: string): boolean {
  const max = data.length;
  let index = 0;
  let hasDigits = false;

  if (!max) return false;

  let ch = data[index];

  // sign
  if (ch === "-" || ch === "+") {
    ch = data[++index];
  }

  if (ch === "0") {
    // 0
    if (index + 1 === max) return true;
    ch = data[++index];

    // base 2, base 8, base 16

    if (ch === "b") {
      // base 2
      index++;

      for (; index < max; index++) {
        ch = data[index];
        if (ch === "_") continue;
        if (ch !== "0" && ch !== "1") return false;
        hasDigits = true;
      }
      return hasDigits && ch !== "_";
    }

    if (ch === "x") {
      // base 16
      index++;

      for (; index < max; index++) {
        ch = data[index];
        if (ch === "_") continue;
        if (!isHexCode(data.charCodeAt(index))) return false;
        hasDigits = true;
      }
      return hasDigits && ch !== "_";
    }

    // base 8
    for (; index < max; index++) {
      ch = data[index];
      if (ch === "_") continue;
      if (!isOctCode(data.charCodeAt(index))) return false;
      hasDigits = true;
    }
    return hasDigits && ch !== "_";
  }

  // base 10 (except 0) or base 60

  // value should not start with `_`;
  if (ch === "_") return false;

  for (; index < max; index++) {
    ch = data[index];
    if (ch === "_") continue;
    if (!isDecCode(data.charCodeAt(index))) {
      return false;
    }
    hasDigits = true;
  }

  // Should have digits and should not end with `_`
  if (!hasDigits || ch === "_") return false;

  // base60 almost not used, no needs to optimize
  return /^(:[0-5]?[0-9])+$/.test(data.slice(index));
}

function constructYamlInteger(data: string): number {
  let value = data;

  if (value.includes("_")) {
    value = value.replace(/_/g, "");
  }

  let sign = 1;
  let ch = value[0];
  if (ch === "-" || ch === "+") {
    if (ch === "-") sign = -1;
    value = value.slice(1);
    ch = value[0];
  }

  if (value === "0") return 0;

  if (ch === "0") {
    if (value[1] === "b") return sign * parseInt(value.slice(2), 2);
    if (value[1] === "x") return sign * parseInt(value, 16);
    return sign * parseInt(value, 8);
  }

  return sign * parseInt(value, 10);
}

function isInteger(object: unknown): object is number {
  if (object instanceof Number) object = object.valueOf();
  return typeof object === "number" && object % 1 === 0 &&
    !isNegativeZero(object);
}

export const int: Type<"scalar", number> = {
  tag: "tag:yaml.org,2002:int",
  construct: constructYamlInteger,
  defaultStyle: "decimal",
  kind: "scalar",
  predicate: isInteger,
  represent: {
    // deno-lint-ignore ban-types
    binary(object: number | Number): string {
      const value = object instanceof Number ? object.valueOf() : object;
      return value >= 0
        ? `0b${value.toString(2)}`
        : `-0b${value.toString(2).slice(1)}`;
    },
    // deno-lint-ignore ban-types
    octal(object: number | Number): string {
      const value = object instanceof Number ? object.valueOf() : object;
      return value >= 0
        ? `0${value.toString(8)}`
        : `-0${value.toString(8).slice(1)}`;
    },
    // deno-lint-ignore ban-types
    decimal(object: number | Number): string {
      const value = object instanceof Number ? object.valueOf() : object;
      return value.toString(10);
    },
    // deno-lint-ignore ban-types
    hexadecimal(object: number | Number): string {
      const value = object instanceof Number ? object.valueOf() : object;
      return value >= 0
        ? `0x${value.toString(16).toUpperCase()}`
        : `-0x${value.toString(16).toUpperCase().slice(1)}`;
    },
  },
  resolve: resolveYamlInteger,
};
