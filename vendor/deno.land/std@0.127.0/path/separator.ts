// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

import { isWindows } from "../_util/os.ts";

export const SEP = isWindows ? "\\" : "/";
export const SEP_PATTERN = isWindows ? /[\\/]+/ : /\/+/;
