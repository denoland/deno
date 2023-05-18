// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

const { ops } = globalThis.Deno.core;

export function uvTranslateSysError(sysErrno: number): string {
  return ops.op_node_sys_to_uv_error(sysErrno);
}
