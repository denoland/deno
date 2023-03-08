// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { isWindows } from "ext:deno_node/_util/os.ts";

export const SEP = isWindows ? "\\" : "/";
export const SEP_PATTERN = isWindows ? /[\\/]+/ : /\/+/;
