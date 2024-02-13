// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

import { assertPath } from "./assert_path.ts";

export function assertArgs(from: string, to: string) {
  assertPath(from);
  assertPath(to);
  if (from === to) return "";
}
