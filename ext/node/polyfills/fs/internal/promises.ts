// Copyright 2018-2026 the Deno authors. MIT license.

import { promisify } from "ext:deno_node/internal/util.mjs";
import { access } from "node:fs";
import type { Buffer } from "node:buffer";

export const accessPromise = promisify(access) as (
  path: string | Buffer | URL,
  mode?: number,
) => Promise<void>;
