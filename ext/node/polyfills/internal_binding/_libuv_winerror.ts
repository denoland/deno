// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { core } from "ext:core/mod.js";
const {
  op_node_sys_to_uv_error,
} = core.ensureFastOps();

export function uvTranslateSysError(sysErrno: number): string {
  return op_node_sys_to_uv_error(sysErrno);
}
