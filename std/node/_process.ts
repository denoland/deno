// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { notImplemented } from "./_utils.ts";

/** https://nodejs.org/api/process.html#process_process_nexttick_callback_args */
function nextTick(this: unknown, cb: () => void): void;
function nextTick<T extends Array<unknown>>(
  this: unknown,
  cb: (...args: T) => void,
  ...args: T
): void;
function nextTick<T extends Array<unknown>>(
  this: unknown,
  cb: (...args: T) => void,
  ...args: T
) {
  if (args) {
    queueMicrotask(() => cb.call(this, ...args));
  } else {
    queueMicrotask(cb);
  }
}

/** https://nodejs.org/api/process.html#process_process */
// @deprecated `import { process } from 'process'` for backwards compatibility with old deno versions
export const process = {
  /** https://nodejs.org/api/process.html#process_process_arch */
  arch: Deno.build.arch,
  /** https://nodejs.org/api/process.html#process_process_argv */
  get argv(): string[] {
    // Getter delegates --allow-env and --allow-read until request
    // Getter also allows the export Proxy instance to function as intended
    return [Deno.execPath(), ...Deno.args];
  },
  /** https://nodejs.org/api/process.html#process_process_chdir_directory */
  chdir: Deno.chdir,
  /** https://nodejs.org/api/process.html#process_process_cwd */
  cwd: Deno.cwd,
  /** https://nodejs.org/api/process.html#process_process_exit_code */
  exit: Deno.exit,
  /** https://nodejs.org/api/process.html#process_process_env */
  get env(): { [index: string]: string } {
    // Getter delegates --allow-env and --allow-read until request
    // Getter also allows the export Proxy instance to function as intended
    return Deno.env.toObject();
  },
  /** https://nodejs.org/api/process.html#process_process_nexttick_callback_args */
  nextTick,
  /** https://nodejs.org/api/process.html#process_process_events */
  // on is not exported by node, it is only available within process:
  // node --input-type=module -e "import { on } from 'process'; console.log(on)"
  // deno-lint-ignore ban-types
  on(_event: string, _callback: Function): void {
    // TODO(rsp): to be implemented
    notImplemented();
  },
  /** https://nodejs.org/api/process.html#process_process_pid */
  pid: Deno.pid,
  /** https://nodejs.org/api/process.html#process_process_platform */
  platform: Deno.build.os === "windows" ? "win32" : Deno.build.os,
  /** https://nodejs.org/api/process.html#process_process_stderr */
  get stderr() {
    return {
      fd: Deno.stderr.rid,
      get isTTY(): boolean {
        return Deno.isatty(this.fd);
      },
      pipe(_destination: Deno.Writer, _options: { end: boolean }): void {
        // TODO(JayHelton): to be implemented
        notImplemented();
      },
      // deno-lint-ignore ban-types
      write(_chunk: string | Uint8Array, _callback: Function): void {
        // TODO(JayHelton): to be implemented
        notImplemented();
      },
      // deno-lint-ignore ban-types
      on(_event: string, _callback: Function): void {
        // TODO(JayHelton): to be implemented
        notImplemented();
      },
    };
  },
  /** https://nodejs.org/api/process.html#process_process_stdin */
  get stdin() {
    return {
      fd: Deno.stdin.rid,
      get isTTY(): boolean {
        return Deno.isatty(this.fd);
      },
      read(_size: number): void {
        // TODO(JayHelton): to be implemented
        notImplemented();
      },
      // deno-lint-ignore ban-types
      on(_event: string, _callback: Function): void {
        // TODO(JayHelton): to be implemented
        notImplemented();
      },
    };
  },
  /** https://nodejs.org/api/process.html#process_process_stdout */
  get stdout() {
    return {
      fd: Deno.stdout.rid,
      get isTTY(): boolean {
        return Deno.isatty(this.fd);
      },
      pipe(_destination: Deno.Writer, _options: { end: boolean }): void {
        // TODO(JayHelton): to be implemented
        notImplemented();
      },
      // deno-lint-ignore ban-types
      write(_chunk: string | Uint8Array, _callback: Function): void {
        // TODO(JayHelton): to be implemented
        notImplemented();
      },
      // deno-lint-ignore ban-types
      on(_event: string, _callback: Function): void {
        // TODO(JayHelton): to be implemented
        notImplemented();
      },
    };
  },
  /** https://nodejs.org/api/process.html#process_process_version */
  version: `v${Deno.version.deno}`,
  /** https://nodejs.org/api/process.html#process_process_versions */
  versions: {
    node: Deno.version.deno,
    ...Deno.version,
  },
};

Object.defineProperty(process, Symbol.toStringTag, {
  enumerable: false,
  writable: true,
  configurable: false,
  value: "process",
});

export default process;
