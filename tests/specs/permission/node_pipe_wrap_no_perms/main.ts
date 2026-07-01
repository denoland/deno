// Regression test: the node-compat PipeWrap handle (the native backing for
// node:net unix sockets, reachable via process.binding("pipe_wrap")) must
// honor Deno's filesystem permission model. Binding a unix-domain socket
// creates a socket inode on disk, so it requires read+write permission for
// the path. Without it, bind() must throw NotCapable, which in turn means the
// path is never recorded and the fchmod/unlink-on-close paths that operate on
// the bound path can never target an arbitrary file.
//
// This process is granted no permissions.

import process from "node:process";

// deno-lint-ignore no-explicit-any
const { Pipe, constants } = (process as any).binding("pipe_wrap");

function tryBind(path: string): string {
  const p = new Pipe(constants.SERVER);
  try {
    p.bind(path);
    return "allowed";
  } catch (e) {
    return (e as Error).name === "NotCapable"
      ? "denied"
      : `other:${(e as Error).name}`;
  }
}

// (a) create: binding a fresh socket path must be denied (no --allow-write),
// so permission-less code cannot create a socket inode on disk.
console.log(
  "create:",
  tryBind("/tmp/deno-pwn.sock") === "denied" ? "PASS" : "FAIL",
);

// (b)/(c): a denied bind must NOT record the path, so a subsequent fchmod or
// close cannot chmod/unlink it. fchmod returns EBADF (< 0) when no path is
// armed; this transitively proves close() has nothing to unlink either.
const q = new Pipe(constants.SERVER);
try {
  q.bind("/etc/hosts");
} catch {
  // expected NotCapable
}
const fchmodRet = q.fchmod(constants.UV_READABLE | constants.UV_WRITABLE);
console.log(
  "chmod-not-armed:",
  fchmodRet < 0 ? "PASS" : "FAIL",
);
