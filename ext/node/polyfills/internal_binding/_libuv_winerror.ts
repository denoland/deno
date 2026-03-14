// Copyright 2018-2026 the Deno authors. MIT license.

import { op_node_sys_to_uv_error } from "ext:core/ops";

export function uvTranslateSysError(sysErrno: number): string {
  return op_node_sys_to_uv_error(sysErrno);
}
