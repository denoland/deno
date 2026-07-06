// Ported from js-yaml v3.13.1:
// https://github.com/nodeca/js-yaml/commit/665aadda42349dcae869f12040d9b10ef18d12da
// Copyright 2011-2015 by Vitaly Puzrin. All rights reserved. MIT license.
// Copyright 2018-2026 the Deno authors. MIT license.

import type { Type } from "../_type.ts";

const YAML_TRUE_BOOLEANS = ["true", "True", "TRUE"];
const YAML_FALSE_BOOLEANS = ["false", "False", "FALSE"];
const YAML_BOOLEANS = [...YAML_TRUE_BOOLEANS, ...YAML_FALSE_BOOLEANS];

export const bool: Type<"scalar", boolean> = {
  tag: "tag:yaml.org,2002:bool",
  kind: "scalar",
  defaultStyle: "lowercase",
  predicate: (value: unknown): value is boolean =>
    typeof value === "boolean" || value instanceof Boolean,
  construct: (data: string): boolean => YAML_TRUE_BOOLEANS.includes(data),
  resolve: (data: string): boolean => YAML_BOOLEANS.includes(data),
  represent: {
    // deno-lint-ignore ban-types
    lowercase: (object: boolean | Boolean): string => {
      const value = object instanceof Boolean ? object.valueOf() : object;
      return value ? "true" : "false";
    },
    // deno-lint-ignore ban-types
    uppercase: (object: boolean | Boolean): string => {
      const value = object instanceof Boolean ? object.valueOf() : object;
      return value ? "TRUE" : "FALSE";
    },
    // deno-lint-ignore ban-types
    camelcase: (object: boolean | Boolean): string => {
      const value = object instanceof Boolean ? object.valueOf() : object;
      return value ? "True" : "False";
    },
  },
};
