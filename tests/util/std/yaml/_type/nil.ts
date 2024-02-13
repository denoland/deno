// Ported from js-yaml v3.13.1:
// https://github.com/nodeca/js-yaml/commit/665aadda42349dcae869f12040d9b10ef18d12da
// Copyright 2011-2015 by Vitaly Puzrin. All rights reserved. MIT license.
// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { Type } from "../type.ts";

function resolveYamlNull(data: string): boolean {
  const max = data.length;

  return (
    (max === 1 && data === "~") ||
    (max === 4 && (data === "null" || data === "Null" || data === "NULL"))
  );
}

function constructYamlNull(): null {
  return null;
}

function isNull(object: unknown): object is null {
  return object === null;
}

export const nil = new Type("tag:yaml.org,2002:null", {
  construct: constructYamlNull,
  defaultStyle: "lowercase",
  kind: "scalar",
  predicate: isNull,
  represent: {
    canonical(): string {
      return "~";
    },
    lowercase(): string {
      return "null";
    },
    uppercase(): string {
      return "NULL";
    },
    camelcase(): string {
      return "Null";
    },
  },
  resolve: resolveYamlNull,
});
