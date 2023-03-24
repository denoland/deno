// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.

const internals = globalThis.__bootstrap.internals;
import { core } from "ext:deno_node/_core.ts";
import { notImplemented, warnNotImplemented } from "ext:deno_node/_utils.ts";
import { EventEmitter } from "ext:deno_node/events.ts";
import { validateString } from "ext:deno_node/internal/validators.mjs";
import {
  ERR_INVALID_ARG_TYPE,
  ERR_UNKNOWN_SIGNAL,
  errnoException,
} from "ext:deno_node/internal/errors.ts";
import { getOptionValue } from "ext:deno_node/internal/options.ts";
import { assert } from "ext:deno_node/_util/asserts.ts";
import { fromFileUrl, join } from "ext:deno_node/path.ts";
import {
  arch as arch_,
  chdir,
  cwd,
  env,
  nextTick as _nextTick,
  version,
  versions,
} from "ext:deno_node/_process/process.ts";
import { _exiting } from "ext:deno_node/_process/exiting.ts";
export { _nextTick as nextTick, chdir, cwd, env, version, versions };
import {
  createWritableStdioStream,
  initStdin,
} from "ext:deno_node/_process/streams.mjs";
import {
  enableNextTick,
  processTicksAndRejections,
  runNextTicks,
} from "ext:deno_node/_next_tick.ts";
import { isWindows } from "ext:deno_node/_util/os.ts";
import * as io from "ext:deno_io/12_io.js";
import { Command } from "ext:runtime/40_process.js";

// TODO(kt3k): This should be set at start up time
export let arch = "";

// TODO(kt3k): This should be set at start up time
export let platform = "";

// TODO(kt3k): This should be set at start up time
export let pid = 0;

// TODO(kt3k): Give better types to stdio objects
// deno-lint-ignore no-explicit-any
let stderr = null as any;
// deno-lint-ignore no-explicit-any
let stdin = null as any;
// deno-lint-ignore no-explicit-any
let stdout = null as any;

export { stderr, stdin, stdout };
import { getBinding } from "ext:deno_node/internal_binding/mod.ts";
import * as constants from "ext:deno_node/internal_binding/constants.ts";
import * as uv from "ext:deno_node/internal_binding/uv.ts";
import type { BindingName } from "ext:deno_node/internal_binding/mod.ts";
import { buildAllowedFlags } from "ext:deno_node/internal/process/per_thread.mjs";

const notImplementedEvents = [
  "disconnect",
  "message",
  "multipleResolves",
  "rejectionHandled",
  "worker",
];

export const argv: string[] = [];

/** https://nodejs.org/api/process.html#process_process_exit_code */
export const exit = (code?: number | string) => {
  if (code || code === 0) {
    if (typeof code === "string") {
      const parsedCode = parseInt(code);
      process.exitCode = isNaN(parsedCode) ? undefined : parsedCode;
    } else {
      process.exitCode = code;
    }
  }

  if (!process._exiting) {
    process._exiting = true;
    // FIXME(bartlomieju): this is wrong, we won't be using syscall to exit
    // and thus the `unload` event will not be emitted to properly trigger "emit"
    // event on `process`.
    process.emit("exit", process.exitCode || 0);
  }

  Deno.exit(process.exitCode || 0);
};

function addReadOnlyProcessAlias(
  name: string,
  option: string,
  enumerable = true,
) {
  const value = getOptionValue(option);

  if (value) {
    Object.defineProperty(process, name, {
      writable: false,
      configurable: true,
      enumerable,
      value,
    });
  }
}

function createWarningObject(
  warning: string,
  type: string,
  code?: string,
  // deno-lint-ignore ban-types
  ctor?: Function,
  detail?: string,
): Error {
  assert(typeof warning === "string");

  // deno-lint-ignore no-explicit-any
  const warningErr: any = new Error(warning);
  warningErr.name = String(type || "Warning");

  if (code !== undefined) {
    warningErr.code = code;
  }
  if (detail !== undefined) {
    warningErr.detail = detail;
  }

  // @ts-ignore this function is not available in lib.dom.d.ts
  Error.captureStackTrace(warningErr, ctor || process.emitWarning);

  return warningErr;
}

function doEmitWarning(warning: Error) {
  process.emit("warning", warning);
}

/** https://nodejs.org/api/process.html#process_process_emitwarning_warning_options */
export function emitWarning(
  warning: string | Error,
  type:
    // deno-lint-ignore ban-types
    | { type: string; detail: string; code: string; ctor: Function }
    | string
    | null,
  code?: string,
  // deno-lint-ignore ban-types
  ctor?: Function,
) {
  let detail;

  if (type !== null && typeof type === "object" && !Array.isArray(type)) {
    ctor = type.ctor;
    code = type.code;

    if (typeof type.detail === "string") {
      detail = type.detail;
    }

    type = type.type || "Warning";
  } else if (typeof type === "function") {
    ctor = type;
    code = undefined;
    type = "Warning";
  }

  if (type !== undefined) {
    validateString(type, "type");
  }

  if (typeof code === "function") {
    ctor = code;
    code = undefined;
  } else if (code !== undefined) {
    validateString(code, "code");
  }

  if (typeof warning === "string") {
    warning = createWarningObject(warning, type as string, code, ctor, detail);
  } else if (!(warning instanceof Error)) {
    throw new ERR_INVALID_ARG_TYPE("warning", ["Error", "string"], warning);
  }

  if (warning.name === "DeprecationWarning") {
    // deno-lint-ignore no-explicit-any
    if ((process as any).noDeprecation) {
      return;
    }

    // deno-lint-ignore no-explicit-any
    if ((process as any).throwDeprecation) {
      // Delay throwing the error to guarantee that all former warnings were
      // properly logged.
      return process.nextTick(() => {
        throw warning;
      });
    }
  }

  process.nextTick(doEmitWarning, warning);
}

export function hrtime(time?: [number, number]): [number, number] {
  const milli = performance.now();
  const sec = Math.floor(milli / 1000);
  const nano = Math.floor(milli * 1_000_000 - sec * 1_000_000_000);
  if (!time) {
    return [sec, nano];
  }
  const [prevSec, prevNano] = time;
  return [sec - prevSec, nano - prevNano];
}

hrtime.bigint = function (): bigint {
  const [sec, nano] = hrtime();
  return BigInt(sec) * 1_000_000_000n + BigInt(nano);
};

export function memoryUsage(): {
  rss: number;
  heapTotal: number;
  heapUsed: number;
  external: number;
  arrayBuffers: number;
} {
  return {
    ...Deno.memoryUsage(),
    arrayBuffers: 0,
  };
}

memoryUsage.rss = function (): number {
  return memoryUsage().rss;
};

// Returns a negative error code than can be recognized by errnoException
function _kill(pid: number, sig: number): number {
  let errCode;

  if (sig === 0) {
    let status;
    if (Deno.build.os === "windows") {
      status = (new Command("powershell.exe", {
        args: ["Get-Process", "-pid", pid],
      })).outputSync();
    } else {
      status = (new Command("kill", {
        args: ["-0", pid],
      })).outputSync();
    }

    if (!status.success) {
      errCode = uv.codeMap.get("ESRCH");
    }
  } else {
    // Reverse search the shortname based on the numeric code
    const maybeSignal = Object.entries(constants.os.signals).find((
      [_, numericCode],
    ) => numericCode === sig);

    if (!maybeSignal) {
      errCode = uv.codeMap.get("EINVAL");
    } else {
      try {
        Deno.kill(pid, maybeSignal[0] as Deno.Signal);
      } catch (e) {
        if (e instanceof TypeError) {
          throw notImplemented(maybeSignal[0]);
        }

        throw e;
      }
    }
  }

  if (!errCode) {
    return 0;
  } else {
    return errCode;
  }
}

export function kill(pid: number, sig: string | number = "SIGTERM") {
  if (pid != (pid | 0)) {
    throw new ERR_INVALID_ARG_TYPE("pid", "number", pid);
  }

  let err;
  if (typeof sig === "number") {
    err = process._kill(pid, sig);
  } else {
    if (sig in constants.os.signals) {
      // @ts-ignore Index previously checked
      err = process._kill(pid, constants.os.signals[sig]);
    } else {
      throw new ERR_UNKNOWN_SIGNAL(sig);
    }
  }

  if (err) {
    throw errnoException(err, "kill");
  }

  return true;
}

// deno-lint-ignore no-explicit-any
function uncaughtExceptionHandler(err: any, origin: string) {
  // The origin parameter can be 'unhandledRejection' or 'uncaughtException'
  // depending on how the uncaught exception was created. In Node.js,
  // exceptions thrown from the top level of a CommonJS module are reported as
  // 'uncaughtException', while exceptions thrown from the top level of an ESM
  // module are reported as 'unhandledRejection'. Deno does not have a true
  // CommonJS implementation, so all exceptions thrown from the top level are
  // reported as 'uncaughtException'.
  process.emit("uncaughtExceptionMonitor", err, origin);
  process.emit("uncaughtException", err, origin);
}

let execPath: string | null = null;

class Process extends EventEmitter {
  constructor() {
    super();
  }

  /** https://nodejs.org/api/process.html#process_process_arch */
  get arch() {
    if (!arch) {
      arch = arch_();
    }
    return arch;
  }

  /**
   * https://nodejs.org/api/process.html#process_process_argv
   * Read permissions are required in order to get the executable route
   */
  argv = argv;

  /** https://nodejs.org/api/process.html#process_process_chdir_directory */
  chdir = chdir;

  /** https://nodejs.org/api/process.html#processconfig */
  config = {
    target_defaults: {},
    variables: {},
  };

  /** https://nodejs.org/api/process.html#process_process_cwd */
  cwd = cwd;

  /**
   * https://nodejs.org/api/process.html#process_process_env
   * Requires env permissions
   */
  env = env;

  /** https://nodejs.org/api/process.html#process_process_execargv */
  execArgv: string[] = [];

  /** https://nodejs.org/api/process.html#process_process_exit_code */
  exit = exit;

  _exiting = _exiting;

  /** https://nodejs.org/api/process.html#processexitcode_1 */
  exitCode: undefined | number = undefined;

  // Typed as any to avoid importing "module" module for types
  // deno-lint-ignore no-explicit-any
  mainModule: any = undefined;

  /** https://nodejs.org/api/process.html#process_process_nexttick_callback_args */
  nextTick = _nextTick;

  /** https://nodejs.org/api/process.html#process_process_events */
  override on(event: "exit", listener: (code: number) => void): this;
  override on(
    event: typeof notImplementedEvents[number],
    // deno-lint-ignore ban-types
    listener: Function,
  ): this;
  // deno-lint-ignore no-explicit-any
  override on(event: string, listener: (...args: any[]) => void): this {
    if (notImplementedEvents.includes(event)) {
      warnNotImplemented(`process.on("${event}")`);
      super.on(event, listener);
    } else if (event.startsWith("SIG")) {
      if (event === "SIGBREAK" && Deno.build.os !== "windows") {
        // Ignores SIGBREAK if the platform is not windows.
      } else if (event === "SIGTERM" && Deno.build.os === "windows") {
        // Ignores SIGTERM on windows.
      } else {
        Deno.addSignalListener(event as Deno.Signal, listener);
      }
    } else {
      super.on(event, listener);
    }

    return this;
  }

  override off(event: "exit", listener: (code: number) => void): this;
  override off(
    event: typeof notImplementedEvents[number],
    // deno-lint-ignore ban-types
    listener: Function,
  ): this;
  // deno-lint-ignore no-explicit-any
  override off(event: string, listener: (...args: any[]) => void): this {
    if (notImplementedEvents.includes(event)) {
      warnNotImplemented(`process.off("${event}")`);
      super.off(event, listener);
    } else if (event.startsWith("SIG")) {
      if (event === "SIGBREAK" && Deno.build.os !== "windows") {
        // Ignores SIGBREAK if the platform is not windows.
      } else if (event === "SIGTERM" && Deno.build.os === "windows") {
        // Ignores SIGTERM on windows.
      } else {
        Deno.removeSignalListener(event as Deno.Signal, listener);
      }
    } else {
      super.off(event, listener);
    }

    return this;
  }

  // deno-lint-ignore no-explicit-any
  override emit(event: string, ...args: any[]): boolean {
    if (event.startsWith("SIG")) {
      if (event === "SIGBREAK" && Deno.build.os !== "windows") {
        // Ignores SIGBREAK if the platform is not windows.
      } else {
        Deno.kill(Deno.pid, event as Deno.Signal);
      }
    } else {
      return super.emit(event, ...args);
    }

    return true;
  }

  override prependListener(
    event: "exit",
    listener: (code: number) => void,
  ): this;
  override prependListener(
    event: typeof notImplementedEvents[number],
    // deno-lint-ignore ban-types
    listener: Function,
  ): this;
  override prependListener(
    event: string,
    // deno-lint-ignore no-explicit-any
    listener: (...args: any[]) => void,
  ): this {
    if (notImplementedEvents.includes(event)) {
      warnNotImplemented(`process.prependListener("${event}")`);
      super.prependListener(event, listener);
    } else if (event.startsWith("SIG")) {
      if (event === "SIGBREAK" && Deno.build.os !== "windows") {
        // Ignores SIGBREAK if the platform is not windows.
      } else {
        Deno.addSignalListener(event as Deno.Signal, listener);
      }
    } else {
      super.prependListener(event, listener);
    }

    return this;
  }

  /** https://nodejs.org/api/process.html#process_process_pid */
  get pid() {
    if (!pid) {
      pid = Deno.pid;
    }
    return pid;
  }

  /** https://nodejs.org/api/process.html#process_process_platform */
  get platform() {
    if (!platform) {
      platform = isWindows ? "win32" : Deno.build.os;
    }
    return platform;
  }

  override addListener(event: "exit", listener: (code: number) => void): this;
  override addListener(
    event: typeof notImplementedEvents[number],
    // deno-lint-ignore ban-types
    listener: Function,
  ): this;
  override addListener(
    event: string,
    // deno-lint-ignore no-explicit-any
    listener: (...args: any[]) => void,
  ): this {
    if (notImplementedEvents.includes(event)) {
      warnNotImplemented(`process.addListener("${event}")`);
    }

    return this.on(event, listener);
  }

  override removeListener(
    event: "exit",
    listener: (code: number) => void,
  ): this;
  override removeListener(
    event: typeof notImplementedEvents[number],
    // deno-lint-ignore ban-types
    listener: Function,
  ): this;
  override removeListener(
    event: string,
    // deno-lint-ignore no-explicit-any
    listener: (...args: any[]) => void,
  ): this {
    if (notImplementedEvents.includes(event)) {
      warnNotImplemented(`process.removeListener("${event}")`);
    }

    return this.off(event, listener);
  }

  /**
   * Returns the current high-resolution real time in a [seconds, nanoseconds]
   * tuple.
   *
   * Note: You need to give --allow-hrtime permission to Deno to actually get
   * nanoseconds precision values. If you don't give 'hrtime' permission, the returned
   * values only have milliseconds precision.
   *
   * `time` is an optional parameter that must be the result of a previous process.hrtime() call to diff with the current time.
   *
   * These times are relative to an arbitrary time in the past, and not related to the time of day and therefore not subject to clock drift. The primary use is for measuring performance between intervals.
   * https://nodejs.org/api/process.html#process_process_hrtime_time
   */
  hrtime = hrtime;

  /**
   * @private
   *
   * NodeJS internal, use process.kill instead
   */
  _kill = _kill;

  /** https://nodejs.org/api/process.html#processkillpid-signal */
  kill = kill;

  memoryUsage = memoryUsage;

  /** https://nodejs.org/api/process.html#process_process_stderr */
  stderr = stderr;

  /** https://nodejs.org/api/process.html#process_process_stdin */
  stdin = stdin;

  /** https://nodejs.org/api/process.html#process_process_stdout */
  stdout = stdout;

  /** https://nodejs.org/api/process.html#process_process_version */
  version = version;

  /** https://nodejs.org/api/process.html#process_process_versions */
  versions = versions;

  /** https://nodejs.org/api/process.html#process_process_emitwarning_warning_options */
  emitWarning = emitWarning;

  binding(name: BindingName) {
    return getBinding(name);
  }

  /** https://nodejs.org/api/process.html#processumaskmask */
  umask() {
    // Always return the system default umask value.
    // We don't use Deno.umask here because it has a race
    // condition bug.
    // See https://github.com/denoland/deno_std/issues/1893#issuecomment-1032897779
    return 0o22;
  }

  /** This method is removed on Windows */
  getgid?(): number {
    return Deno.gid()!;
  }

  /** This method is removed on Windows */
  getuid?(): number {
    return Deno.uid()!;
  }

  // TODO(kt3k): Implement this when we added -e option to node compat mode
  _eval: string | undefined = undefined;

  /** https://nodejs.org/api/process.html#processexecpath */
  get execPath() {
    if (execPath) {
      return execPath;
    }
    execPath = Deno.execPath();
    return execPath;
  }

  set execPath(path: string) {
    execPath = path;
  }

  setStartTime(t: number) {
    this.#startTime = t;
  }

  #startTime = 0;
  /** https://nodejs.org/api/process.html#processuptime */
  uptime() {
    return (Date.now() - this.#startTime) / 1000;
  }

  #allowedFlags = buildAllowedFlags();
  /** https://nodejs.org/api/process.html#processallowednodeenvironmentflags */
  get allowedNodeEnvironmentFlags() {
    return this.#allowedFlags;
  }

  features = { inspector: false };

  // TODO(kt3k): Get the value from --no-deprecation flag.
  noDeprecation = false;
}

if (isWindows) {
  delete Process.prototype.getgid;
  delete Process.prototype.getuid;
}

/** https://nodejs.org/api/process.html#process_process */
const process = new Process();

Object.defineProperty(process, Symbol.toStringTag, {
  enumerable: false,
  writable: true,
  configurable: false,
  value: "process",
});

addReadOnlyProcessAlias("noDeprecation", "--no-deprecation");
addReadOnlyProcessAlias("throwDeprecation", "--throw-deprecation");

export const removeListener = process.removeListener;
export const removeAllListeners = process.removeAllListeners;

// Should be called only once, in `runtime/js/99_main.js` when the runtime is
// bootstrapped.
internals.__bootstrapNodeProcess = function (
  argv0: string | undefined,
  args: string[],
  denoVersions: Record<string, string>,
) {
  // Overwrites the 1st item with getter.
  if (typeof argv0 === "string") {
    Object.defineProperty(argv, "0", {
      get: () => {
        return argv0;
      },
    });
  } else {
    Object.defineProperty(argv, "0", {
      get: () => {
        return Deno.execPath();
      },
    });
  }

  // Overwrites the 2st item with getter.
  Object.defineProperty(argv, "1", {
    get: () => {
      if (Deno.mainModule.startsWith("file:")) {
        return fromFileUrl(Deno.mainModule);
      } else {
        return join(Deno.cwd(), "$deno$node.js");
      }
    },
  });
  for (let i = 0; i < args.length; i++) {
    argv[i + 2] = args[i];
  }

  for (const [key, value] of Object.entries(denoVersions)) {
    versions[key] = value;
  }

  core.setNextTickCallback(processTicksAndRejections);
  core.setMacrotaskCallback(runNextTicks);
  enableNextTick();

  // TODO(bartlomieju): this is buggy, see https://github.com/denoland/deno/issues/16928
  // We should use a specialized API in 99_main.js instead
  globalThis.addEventListener("unhandledrejection", (event) => {
    if (process.listenerCount("unhandledRejection") === 0) {
      // The Node.js default behavior is to raise an uncaught exception if
      // an unhandled rejection occurs and there are no unhandledRejection
      // listeners.
      if (process.listenerCount("uncaughtException") === 0) {
        throw event.reason;
      }

      event.preventDefault();
      uncaughtExceptionHandler(event.reason, "unhandledRejection");
      return;
    }

    event.preventDefault();
    process.emit("unhandledRejection", event.reason, event.promise);
  });

  globalThis.addEventListener("error", (event) => {
    if (process.listenerCount("uncaughtException") > 0) {
      event.preventDefault();
    }

    uncaughtExceptionHandler(event.error, "uncaughtException");
  });

  globalThis.addEventListener("beforeunload", (e) => {
    process.emit("beforeExit", process.exitCode || 0);
    processTicksAndRejections();
    if (core.eventLoopHasMoreWork()) {
      e.preventDefault();
    }
  });

  globalThis.addEventListener("unload", () => {
    if (!process._exiting) {
      process._exiting = true;
      process.emit("exit", process.exitCode || 0);
    }
  });

  // Initializes stdin
  stdin = process.stdin = initStdin();

  /** https://nodejs.org/api/process.html#process_process_stderr */
  stderr = process.stderr = createWritableStdioStream(
    io.stderr,
    "stderr",
  );

  /** https://nodejs.org/api/process.html#process_process_stdout */
  stdout = process.stdout = createWritableStdioStream(
    io.stdout,
    "stdout",
  );

  process.setStartTime(Date.now());
  // @ts-ignore Remove setStartTime and #startTime is not modifiable
  delete process.setStartTime;
  delete internals.__bootstrapNodeProcess;
};

export default process;
