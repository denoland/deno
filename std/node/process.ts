import { notImplemented } from "./_utils.ts";

/** https://nodejs.org/api/process.html#process_process_arch */
export const arch = Deno.build.arch;

/** https://nodejs.org/api/process.html#process_process_chdir_directory */
export const chdir = Deno.chdir;

/** https://nodejs.org/api/process.html#process_process_cwd */
export const cwd = Deno.cwd;

/** https://nodejs.org/api/process.html#process_process_exit_code */
export const exit = Deno.exit;

/** https://nodejs.org/api/process.html#process_process_pid */
export const pid = Deno.pid;

/** https://nodejs.org/api/process.html#process_process_platform */
export const platform = Deno.build.os === "windows" ? "win32" : Deno.build.os;

/** https://nodejs.org/api/process.html#process_process_version */
export const version = `v${Deno.version.deno}`;

/** https://nodejs.org/api/process.html#process_process_versions */
export const versions = {
  node: Deno.version.deno,
  ...Deno.version,
};

/** https://nodejs.org/api/process.html#process_process */
// @deprecated exported only for backwards compatibility with old deno versions
export const process = {
  arch,
  chdir,
  cwd,
  exit,
  pid,
  platform,
  version,
  versions,

  /** https://nodejs.org/api/process.html#process_process_events */
  // node --input-type=module -e "import {on} from 'process'; console.log(on)"
  // on is not exported by node, it is only available within process
  on(_event: string, _callback: Function): void {
    // TODO(rsp): to be implemented
    notImplemented();
  },

  /** https://nodejs.org/api/process.html#process_process_env */
  get env(): { [index: string]: string } {
    // using getter to avoid --allow-env unless it's used
    return Deno.env.toObject();
  },

  /** https://nodejs.org/api/process.html#process_process_argv */
  get argv(): string[] {
    // Deno.execPath() also requires --allow-env
    return [Deno.execPath(), ...Deno.args];
  },
};

// define the type for configuring the env and argv promises
// as well as for the global.process declaration
type Process = typeof process;

/** requires the use of await for compatibility with deno */
export const env = new Promise<Process["env"]>((resolve) =>
  resolve(process.env)
);

/** requires the use of await for compatibility with deno */
export const argv = new Promise<Process["argv"]>((resolve) =>
  resolve(process.argv)
);

/** use this for access to `process.env` and `process.argv` without the need for await */
export default process;

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

declare global {
  const process: Process;
}
