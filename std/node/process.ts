import { notImplemented } from "./_utils.ts";

function on(_event: string, _callback: Function): void {
  // TODO(rsp): to be implemented
  notImplemented();
}

export const process = {
  version: `v${Deno.version.deno}`,
  versions: {
    node: Deno.version.deno,
    ...Deno.version,
  },
  platform: Deno.build.os === "windows" ? "win32" : Deno.build.os,
  arch: Deno.build.arch,
  pid: Deno.pid,
  cwd: Deno.cwd,
  chdir: Deno.chdir,
  exit: Deno.exit,
  on,
  get env(): { [index: string]: string } {
    // using getter to avoid --allow-env unless it's used
    return Deno.env.toObject();
  },
  get argv(): string[] {
    // Deno.execPath() also requires --allow-env
    return [Deno.execPath(), ...Deno.args];
  },
};

Object.defineProperty(process, Symbol.toStringTag, {
  enumerable: false,
  writable: true,
  configurable: false,
  value: "process",
});

Object.defineProperty(globalThis, "process", {
  value: process,
  enumerable: false,
  writable: true,
  configurable: true,
});
