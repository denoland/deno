// Ported from js-yaml v3.13.1:
// https://github.com/nodeca/js-yaml/commit/665aadda42349dcae869f12040d9b10ef18d12da
// Copyright 2011-2015 by Vitaly Puzrin. All rights reserved. MIT license.
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { Type } from "../type.ts";
import type { Any } from "../utils.ts";

const _toString = Object.prototype.toString;

function resolveYamlPairs(data: Any[][]): boolean {
  const result = new Array(data.length);

  for (let index = 0; index < data.length; index++) {
    const pair = data[index];

    if (_toString.call(pair) !== "[object Object]") return false;

    const keys = Object.keys(pair);

    if (keys.length !== 1) return false;

    result[index] = [keys[0], pair[keys[0] as Any]];
  }

  return true;
}

function constructYamlPairs(data: string): Any[] {
  if (data === null) return [];

  const result = new Array(data.length);

  for (let index = 0; index < data.length; index += 1) {
    const pair = data[index];

    const keys = Object.keys(pair);

    result[index] = [keys[0], pair[keys[0] as Any]];
  }

  return result;
}

export const pairs = new Type("tag:yaml.org,2002:pairs", {
  construct: constructYamlPairs,
  kind: "sequence",
  resolve: resolveYamlPairs,
});
