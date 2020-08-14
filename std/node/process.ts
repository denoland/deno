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
// @deprecated `import { process } from 'process'` for backwards compatibility with old deno versions
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
  // on is not exported by node, it is only available within process:
  // node --input-type=module -e "import { on } from 'process'; console.log(on)"
  on(_event: string, _callback: Function): void {
    // TODO(rsp): to be implemented
    notImplemented();
  },

  /** https://nodejs.org/api/process.html#process_process_argv */
  get argv(): string[] {
    // Getter delegates --allow-env and --allow-read until request
    // Getter also allows the export Proxy instance to function as intended
    return [Deno.execPath(), ...Deno.args];
  },

  /** https://nodejs.org/api/process.html#process_process_env */
  get env(): { [index: string]: string } {
    // Getter delegates --allow-env and --allow-read until request
    // Getter also allows the export Proxy instance to function as intended
    return Deno.env.toObject();
  },
};

/**
 * https://nodejs.org/api/process.html#process_process_argv
 * @example `import { argv } from './std/node/process.ts'; console.log(argv)`
 */
// Proxy delegates --allow-env and --allow-read to request time, even for exports
export const argv = new Proxy(process.argv, {});

/**
 * https://nodejs.org/api/process.html#process_process_env
 * @example `import { env } from './std/node/process.ts'; console.log(env)`
 */
// Proxy delegates --allow-env and --allow-read to request time, even for exports
export const env = new Proxy(process.env, {});

// import process from './std/node/process.ts'
export default process;

// Define the type for the global declration
type Process = typeof process;

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
