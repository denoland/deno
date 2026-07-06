// Ported from js-yaml v3.13.1:
// https://github.com/nodeca/js-yaml/commit/665aadda42349dcae869f12040d9b10ef18d12da
// Copyright 2018-2026 the Deno authors. MIT license.

import type { Type } from "../_type.ts";

export const str: Type<"scalar", string> = {
  tag: "tag:yaml.org,2002:str",
  kind: "scalar",
  resolve: (): boolean => true,
  construct: (data: string | null): string => data !== null ? data : "",
};
