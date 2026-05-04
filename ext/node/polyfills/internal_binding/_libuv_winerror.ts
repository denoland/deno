// Copyright 2018-2026 the Deno authors. MIT license.
// deno-fmt-ignore-file

(function () {
const { core } = globalThis.__bootstrap;
const { op_node_sys_to_uv_error } = core.ops;

function uvTranslateSysError(sysErrno: number): string {
  return op_node_sys_to_uv_error(sysErrno);
}

return { uvTranslateSysError };
})()
