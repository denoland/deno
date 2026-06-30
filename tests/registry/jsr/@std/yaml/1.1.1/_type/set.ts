// Ported from js-yaml v3.13.1:
// https://github.com/nodeca/js-yaml/commit/665aadda42349dcae869f12040d9b10ef18d12da
// Copyright 2011-2015 by Vitaly Puzrin. All rights reserved. MIT license.
// Copyright 2018-2026 the Deno authors. MIT license.

import type { Type } from "../_type.ts";

export const set: Type<"mapping", Record<PropertyKey, unknown>> = {
  tag: "tag:yaml.org,2002:set",
  kind: "mapping",
  construct: (data: Record<string, unknown>): Record<string, unknown> =>
    data !== null ? data : {},
  resolve: (data: Record<string, unknown>): boolean => {
    if (data === null) return true;
    return Object.values(data).every((it) => it === null);
  },
};
