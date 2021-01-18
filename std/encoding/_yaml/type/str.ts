// Ported from js-yaml v3.13.1:
// https://github.com/nodeca/js-yaml/commit/665aadda42349dcae869f12040d9b10ef18d12da
// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

import { Type } from "../type.ts";

export const str = new Type("tag:yaml.org,2002:str", {
  construct(data): string {
    return data !== null ? data : "";
  },
  kind: "scalar",
});
