// Regression test: the node-compat PipeWrap handle (the native backing for
// node:net unix sockets, reachable via process.binding("pipe_wrap")) must
// honor Deno's *network* permission model, not just the filesystem one.
//
// A unix-domain socket is both a filesystem entry and an outbound network
// primitive. The native path (`Deno.connect/listen({ transport: "unix" })`)
// requires an `--allow-net=unix:<path>` grant *in addition to* filesystem
// access, so that read/write on e.g. `/var/run/docker.sock` alone cannot reach
// local IPC services (Docker, dbus, podman, custom RPC) under `--deny-net`.
//
// This process is granted full `--allow-read`/`--allow-write` but `--deny-net`.
// With filesystem access satisfied, connect()/bind()/listen() must STILL be
// denied because the unix-socket network grant is missing.

import process from "node:process";

// deno-lint-ignore no-explicit-any
const { Pipe, constants } = (process as any).binding("pipe_wrap");

const SOCK = "/tmp/deno-pipewrap-net-perm.sock";

function classify(fn: () => void): string {
  try {
    fn();
    return "allowed";
  } catch (e) {
    return (e as Error).name === "NotCapable"
      ? "denied"
      : `other:${(e as Error).name}`;
  }
}

// connect(): outbound unix-socket connection must require --allow-net=unix.
console.log(
  "connect:",
  classify(() => {
      new Pipe(constants.SOCKET).connect({}, SOCK);
    }) === "denied"
    ? "PASS"
    : "FAIL",
);

// bind(): creating a listening unix socket must require --allow-net=unix.
console.log(
  "bind:",
  classify(() => {
      new Pipe(constants.SERVER).bind(SOCK);
    }) === "denied"
    ? "PASS"
    : "FAIL",
);
