// Ported from js-yaml v3.13.1:
// https://github.com/nodeca/js-yaml/commit/665aadda42349dcae869f12040d9b10ef18d12da
// Copyright 2011-2015 by Vitaly Puzrin. All rights reserved. MIT license.
// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

import { Type } from "../type.ts";
import { isBoolean } from "../utils.ts";

function resolveYamlBoolean(data: string): boolean {
  const max = data.length;

  return (
    (max === 4 && (data === "true" || data === "True" || data === "TRUE")) ||
    (max === 5 && (data === "false" || data === "False" || data === "FALSE"))
  );
}

function constructYamlBoolean(data: string): boolean {
  return data === "true" || data === "True" || data === "TRUE";
}

export const bool = new Type("tag:yaml.org,2002:bool", {
  construct: constructYamlBoolean,
  defaultStyle: "lowercase",
  kind: "scalar",
  predicate: isBoolean,
  represent: {
    lowercase(object: boolean): string {
      return object ? "true" : "false";
    },
    uppercase(object: boolean): string {
      return object ? "TRUE" : "FALSE";
    },
    camelcase(object: boolean): string {
      return object ? "True" : "False";
    },
  },
  resolve: resolveYamlBoolean,
});
