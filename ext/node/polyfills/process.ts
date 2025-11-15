// Copyright 2018-2025 the Deno authors. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import { core, internals, primordials } from "ext:core/mod.js";
import { initializeDebugEnv } from "ext:deno_node/internal/util/debuglog.ts";
import { format } from "ext:deno_node/internal/util/inspect.mjs";
import {
  op_getegid,
  op_geteuid,
  op_node_load_env_file,
  op_node_process_kill,
  op_node_process_setegid,
  op_node_process_seteuid,
  op_node_process_setgid,
  op_node_process_setuid,
  op_process_abort,
} from "ext:core/ops";

import { warnNotImplemented } from "ext:deno_node/_utils.ts";
import { EventEmitter } from "node:events";
import Module, { getBuiltinModule } from "node:module";
import { report } from "ext:deno_node/internal/process/report.ts";
import { onWarning } from "ext:deno_node/internal/process/warning.ts";
import {
  validateNumber,
  validateObject,
  validateString,
  validateUint32,
} from "ext:deno_node/internal/validators.mjs";
import {
  denoErrorToNodeError,
  ERR_INVALID_ARG_TYPE,
  ERR_INVALID_ARG_VALUE_RANGE,
  ERR_OUT_OF_RANGE,
  ERR_UNKNOWN_SIGNAL,
  errnoException,
} from "ext:deno_node/internal/errors.ts";
import { getOptionValue } from "ext:deno_node/internal/options.ts";
import { assert } from "ext:deno_node/_util/asserts.ts";
import { join } from "node:path";
import { pathFromURL } from "ext:deno_web/00_infra.js";
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
export {
  _nextTick as nextTick,
  chdir,
  cwd,
  env,
  getBuiltinModule,
  version,
  versions,
};
import {
  createWritableStdioStream,
  initStdin,
} from "ext:deno_node/_process/streams.mjs";
import {
  enableNextTick,
  processTicksAndRejections,
  runNextTicks,
} from "ext:deno_node/_next_tick.ts";
import { isAndroid, isWindows } from "ext:deno_node/_util/os.ts";
import * as io from "ext:deno_io/12_io.js";
import * as denoOs from "ext:deno_os/30_os.js";

export let argv0 = "";
export let arch = "";
export let platform = isWindows ? "win32" : ""; // initialized during bootstrap
export let pid = 0;
export let ppid = 0;

let stdin, stdout, stderr;
export { stderr, stdin, stdout };

import { getBinding } from "ext:deno_node/internal_binding/mod.ts";
import * as constants from "ext:deno_node/internal_binding/constants.ts";
import * as uv from "ext:deno_node/internal_binding/uv.ts";
import type { BindingName } from "ext:deno_node/internal_binding/mod.ts";
import { buildAllowedFlags } from "ext:deno_node/internal/process/per_thread.mjs";

const { NumberMAX_SAFE_INTEGER } = primordials;

const notImplementedEvents = [
  "multipleResolves",
];

export const argv: string[] = ["", ""]; // Now always string array

// In Node, `process.exitCode` is initially `undefined` until set.
let ProcessExitCode: undefined | null | string | number;

export const execArgv: string[] = [];

/** [https://nodejs.org/api/process.html#process_process_exit_code](https://nodejs.org/api/process.html#process_process_exit_code) */
export const exit = (code?: number | string) => {
  if (code || code === 0) {
    process.exitCode = code;
  } else if (Number.isNaN(code)) {
    process.exitCode = 1;
  }
  ProcessExitCode = denoOs.getExitCode();
  if (!process._exiting) {
    process._exiting = true;
    process.emit("exit", ProcessExitCode);
  }
  process.reallyExit(ProcessExitCode);
};

export const umask = () => {
  return 0o22;
};

export const abort = () => {
  op_process_abort();
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

interface CpuUsage {
  user: number;
  system: number;
}

function previousCpuUsageValueIsValid(num) {
  return typeof num === "number" && num >= 0 && num <= NumberMAX_SAFE_INTEGER;
}

export function cpuUsage(previousValue?: CpuUsage): CpuUsage {
  const cpuValues = Deno.cpuUsage(previousValue);
  if (previousValue) {
    if (!previousCpuUsageValueIsValid(previousValue.user)) {
      validateObject(previousValue, "prevValue");
      validateNumber(previousValue.user, "prevValue.user");
      throw new ERR_INVALID_ARG_VALUE_RANGE(
        "prevValue.user",
        previousValue.user,
      );
    }
    if (!previousCpuUsageValueIsValid(previousValue.system)) {
      validateNumber(previousValue.system, "prevValue.system");
      throw new ERR_INVALID_ARG_VALUE_RANGE(
        "prevValue.system",
        previousValue.system,
      );
    }
    return {
      user: cpuValues.user - previousValue.user,
      system: cpuValues.system - previousValue.system,
    };
  }
  return cpuValues;
}

function createWarningObject(
  warning: string,
  type: string,
  code?: string,
  ctor?: Function,
  detail?: string,
): Error {
  assert(typeof warning === "string");
  const warningErr: any = new Error(warning);
  warningErr.name = String(type || "Warning");
  if (code !== undefined) {
    warningErr.code = code;
  }
  if (detail !== undefined) {
    warningErr.detail = detail;
  }
  Error.captureStackTrace(warningErr, ctor || process.emitWarning);
  return warningErr;
}

function doEmitWarning(warning: Error) {
  process.emit("warning", warning);
}

export function emitWarning(
  warning: string | Error,
  type:
    | { type: string; detail: string; code: string; ctor: Function }
    | string
    | null,
  code?: string,
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
    if ((process as any).noDeprecation) {
      return;
    }
    if ((process as any).throwDeprecation) {
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

function _kill(pid: number, sig: number): number {
  const maybeMapErrno = (res: number) =>
    res === 0 ? res : isWindows ? res : uv.mapSysErrnoToUvErrno(res);
  if (sig === 0) {
    return maybeMapErrno(op_node_process_kill(pid, 0));
  }
  const maybeSignal = Object.entries(constants.os.signals).find((
    [_, numericCode],
  ) => numericCode === sig);
  if (!maybeSignal) {
    return uv.codeMap.get("EINVAL");
  }
  return maybeMapErrno(op_node_process_kill(pid, sig));
}

export function dlopen(module, filename, _flags) {
  Module._extensions[".node"](module, filename);
  return module;
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

let getgid, getuid, getegid, geteuid, setegid, seteuid, setgid, setuid;

function wrapIdSetter(
  syscall: string,
  fn: (id: number | string) => void,
): (id: number | string) => void {
  return function (id: number | string) {
    if (typeof id === "number") {
      validateUint32(id, "id");
      id >>>= 0;
    } else if (typeof id !== "string") {
      throw new ERR_INVALID_ARG_TYPE("id", ["number", "string"], id);
    }
    try {
      fn(id);
    } catch (err) {
      throw denoErrorToNodeError(err as Error, { syscall });
    }
  };
}

if (!isWindows) {
  getgid = () => Deno.gid();
  getuid = () => Deno.uid();
  getegid = () => op_getegid();
  geteuid = () => op_geteuid();
  if (!isAndroid) {
    setegid = wrapIdSetter("setegid", op_node_process_setegid);
    seteuid = wrapIdSetter("seteuid", op_node_process_seteuid);
    setgid = wrapIdSetter("setgid", op_node_process_setgid);
    setuid = wrapIdSetter("setuid", op_node_process_setuid);
  }
}

export { getegid, geteuid, getgid, getuid, setegid, seteuid, setgid, setuid };

const ALLOWED_FLAGS = buildAllowedFlags();

function uncaughtExceptionHandler(err: any, origin: string) {
  process.emit("uncaughtExceptionMonitor", err, origin);
  process.emit("uncaughtException", err, origin);
}

export let execPath: string = Object.freeze({
  __proto__: String.prototype,
  toString() {
    execPath = Deno.execPath();
    return execPath;
  },
  get length() {
    return this.toString().length;
  },
  [Symbol.for("Deno.customInspect")](inspect, options) {
    return inspect(this.toString(), options);
  },
}) as any as string;

// The process class needs to be an ES5 class because it can be instantiated
// in Node without the `new` keyword. It's not a true class in Node.
function Process(this: any) {
  if (!(this instanceof Process)) return new (Process as any)();
  EventEmitter.call(this);
}
Process.prototype = Object.create(EventEmitter.prototype);

Process.prototype.on = function (
  this: any,
  event: string,
  listener: (...args: any[]) => void,
) {
  if (notImplementedEvents.includes(event)) {
    warnNotImplemented(`process.on("${event}")`);
    EventEmitter.prototype.on.call(this, event, listener);
  } else if (event.startsWith("SIG")) {
    if (event === "SIGBREAK" && Deno.build.os !== "windows") {
    } else if (event === "SIGTERM" && Deno.build.os === "windows") {
    } else if (
      event !== "SIGBREAK" && event !== "SIGINT" && Deno.build.os === "windows"
    ) {
    } else {
      EventEmitter.prototype.on.call(this, event, listener);
      Deno.addSignalListener(event as Deno.Signal, listener);
    }
  } else {
    EventEmitter.prototype.on.call(this, event, listener);
  }
  return this;
};

Process.prototype.off = function (
  this: any,
  event: string,
  listener: (...args: any[]) => void,
) {
  if (notImplementedEvents.includes(event)) {
    warnNotImplemented(`process.off("${event}")`);
    EventEmitter.prototype.off.call(this, event, listener);
  } else if (event.startsWith("SIG")) {
    if (event === "SIGBREAK" && Deno.build.os !== "windows") {
    } else if (
      event !== "SIGBREAK" && event !== "SIGINT" && Deno.build.os === "windows"
    ) {
    } else {
      EventEmitter.prototype.off.call(this, event, listener);
      Deno.removeSignalListener(event as Deno.Signal, listener);
    }
  } else {
    EventEmitter.prototype.off.call(this, event, listener);
  }
  return this;
};

Process.prototype.emit = function (
  this: any,
  event: string,
  ...args: any[]
): boolean {
  if (event.startsWith("SIG")) {
    if (event === "SIGBREAK" && Deno.build.os !== "windows") {
    } else {
      Deno.kill(Deno.pid, event as Deno.Signal);
    }
  } else {
    return EventEmitter.prototype.emit.call(this, event, ...args);
  }
  return true;
};

Process.prototype.prependListener = function (
  this: any,
  event: string,
  listener: (...args: any[]) => void,
) {
  if (notImplementedEvents.includes(event)) {
    warnNotImplemented(`process.prependListener("${event}")`);
    EventEmitter.prototype.prependListener.call(this, event, listener);
  } else if (event.startsWith("SIG")) {
    if (event === "SIGBREAK" && Deno.build.os !== "windows") {
    } else {
      EventEmitter.prototype.prependListener.call(this, event, listener);
      Deno.addSignalListener(event as Deno.Signal, listener);
    }
  } else {
    EventEmitter.prototype.prependListener.call(this, event, listener);
  }
  return this;
};

Process.prototype.addListener = function (
  this: any,
  event: string,
  listener: (...args: any[]) => void,
) {
  if (notImplementedEvents.includes(event)) {
    warnNotImplemented(`process.addListener("${event}")`);
  }
  return this.on(event, listener);
};

Process.prototype.removeListener = function (
  this: any,
  event: string,
  listener: (...args: any[]) => void,
) {
  if (notImplementedEvents.includes(event)) {
    warnNotImplemented(`process.removeListener("${event}")`);
  }
  return this.off(event, listener);
};

// @ts-ignore TS doesn't work well with ES5 classes
const process = new Process();

Object.defineProperty(process, "release", {
  get() {
    return {
      name: "node",
      sourceUrl:
        `https://nodejs.org/download/release/${version}/node-${version}.tar.gz`,
      headersUrl:
        `https://nodejs.org/download/release/${version}/node-${version}-headers.tar.gz`,
    };
  },
});

Object.defineProperty(process, "arch", {
  get() {
    return arch;
  },
});

Object.defineProperty(process, "report", {
  get() {
    return report;
  },
});

Object.defineProperty(process, "title", {
  get() {
    return "deno";
  },
  set(_value) {},
});

/**
 * [https://nodejs.org/api/process.html#process_process_argv](https://nodejs.org/api/process.html#process_process_argv)
 * Read permissions are required in order to get the executable route
 */
process.argv = argv;

Object.defineProperty(process, "argv0", {
  get() {
    return argv0;
  },
  set(_val) {},
});

process.chdir = chdir;
process.config = {
  target_defaults: {
    default_configuration: "Release",
  },
  variables: {
    llvm_version: "0.0",
    enable_lto: "false",
  },
};

process.cpuUsage = cpuUsage;
process.cwd = cwd;
process.env = env;
process.execArgv = execArgv;
process.exit = exit;
process.abort = abort;
process._rawDebug = (...args: unknown[]) => {
  core.print(`${format(...args)}\n`, true);
};
process.reallyExit = (code: number) => {
  return Deno.exit(code || 0);
};
process._exiting = _exiting;

Object.defineProperty(process, "exitCode", {
  get() {
    return ProcessExitCode;
  },
  set(code: number | string | null | undefined) {
    let parsedCode: number;
    if (code == null) {
      parsedCode = 0;
    } else if (typeof code === "number") {
      parsedCode = code;
    } else if (typeof code === "string") {
      parsedCode = Number(code);
    } else {
      throw new ERR_INVALID_ARG_TYPE("code", "number", code);
    }
    if (!Number.isInteger(parsedCode)) {
      throw new ERR_OUT_OF_RANGE("code", "an integer", parsedCode);
    }
    denoOs.setExitCode(parsedCode);
    ProcessExitCode = code;
  },
});

process.mainModule = undefined;
process.nextTick = _nextTick;
process.dlopen = dlopen;

Object.defineProperty(process, "pid", {
  get() {
    return pid;
  },
});

Object.defineProperty(process, "ppid", {
  get() {
    return Deno.ppid;
  },
});

Object.defineProperty(process, "platform", {
  get() {
    return platform;
  },
});

process.setSourceMapsEnabled = (_val: boolean) => {};

process.hrtime = hrtime;

process._kill = _kill;
process.kill = kill;
process.memoryUsage = memoryUsage;
process.stderr = stderr;
process.stdin = stdin;
process.stdout = stdout;
process.version = version;
process.versions = versions;
process.emitWarning = emitWarning;
process.binding = (name: BindingName) => {
  return getBinding(name);
};
process.umask = () => {
  return 0o22;
};
process.getgid = getgid;
process.getuid = getuid;
process.getegid = getegid;
process.geteuid = geteuid;
process.setegid = setegid;
process.seteuid = seteuid;
process.setgid = setgid;
process.setuid = setuid;
process.getBuiltinModule = getBuiltinModule;
process._eval = undefined;

export function loadEnvFile(path = ".env") {
  return op_node_load_env_file(path);
}

process.loadEnvFile = loadEnvFile;

Object.defineProperty(process, "execPath", {
  get() {
    return String(execPath);
  },
  set(path: string) {
    execPath = path;
  },
});

process.uptime = () => {
  return Number((performance.now() / 1000).toFixed(9));
};

Object.defineProperty(process, "allowedNodeEnvironmentFlags", {
  get() {
    return ALLOWED_FLAGS;
  },
});

export const allowedNodeEnvironmentFlags = ALLOWED_FLAGS;
process.features = { inspector: false };
process.noDeprecation = false;
process.moduleLoadList = [];

if (isWindows) {
  delete process.getgid;
  delete process.getuid;
  delete process.getegid;
  delete process.geteuid;
}

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

let unhandledRejectionListenerCount = 0;
let rejectionHandledListenerCount = 0;
let uncaughtExceptionListenerCount = 0;
let beforeExitListenerCount = 0;
let exitListenerCount = 0;

process.on("newListener", (event: string) => {
  switch (event) {
    case "unhandledRejection":
      unhandledRejectionListenerCount++;
      break;
    case "rejectionHandled":
      rejectionHandledListenerCount++;
      break;
    case "uncaughtException":
      uncaughtExceptionListenerCount++;
      break;
    case "beforeExit":
      beforeExitListenerCount++;
      break;
    case "exit":
      exitListenerCount++;
      break;
    default:
      return;
  }
  synchronizeListeners();
});

process.on("removeListener", (event: string) => {
  switch (event) {
    case "unhandledRejection":
      unhandledRejectionListenerCount--;
      break;
    case "rejectionHandled":
      rejectionHandledListenerCount--;
      break;
    case "uncaughtException":
      uncaughtExceptionListenerCount--;
      break;
    case "beforeExit":
      beforeExitListenerCount--;
      break;
    case "exit":
      exitListenerCount--;
      break;
    default:
      return;
  }
  synchronizeListeners();
});

function processOnError(event: ErrorEvent) {
  if (process.listenerCount("uncaughtException") > 0) {
    event.preventDefault();
  }
  uncaughtExceptionHandler(event.error, "uncaughtException");
}

function dispatchProcessBeforeExitEvent() {
  process.emit("beforeExit", process.exitCode || 0);
  processTicksAndRejections();
  return core.eventLoopHasMoreWork();
}

function dispatchProcessExitEvent() {
  if (!process._exiting) {
    process._exiting = true;
    process.emit("exit", process.exitCode || 0);
  }
}

function synchronizeListeners() {
  if (
    unhandledRejectionListenerCount > 0 || uncaughtExceptionListenerCount > 0
  ) {
    internals.nodeProcessUnhandledRejectionCallback = (event) => {
      if (process.listenerCount("unhandledRejection") === 0) {
        event.preventDefault();
        uncaughtExceptionHandler(event.reason, "unhandledRejection");
        return;
      }
      event.preventDefault();
      process.emit("unhandledRejection", event.reason, event.promise);
    };
  } else {
    internals.nodeProcessUnhandledRejectionCallback = undefined;
  }
  if (rejectionHandledListenerCount > 0) {
    internals.nodeProcessRejectionHandledCallback = (event) => {
      process.emit("rejectionHandled", event.reason, event.promise);
    };
  } else {
    internals.nodeProcessRejectionHandledCallback = undefined;
  }
  if (uncaughtExceptionListenerCount > 0) {
    globalThis.addEventListener("error", processOnError);
  } else {
    globalThis.removeEventListener("error", processOnError);
  }
}

internals.dispatchProcessBeforeExitEvent = dispatchProcessBeforeExitEvent;
internals.dispatchProcessExitEvent = dispatchProcessExitEvent;
internals.__bootstrapNodeProcess = function (
  argv0Val: string | undefined,
  args: string[],
  denoVersions: Record<string, string>,
  nodeDebug: string,
  warmup = false,
) {
  if (!warmup) {
    argv0 = argv0Val || "";

    // Initialize argv[0] and argv[1] as concrete strings
    argv[0] = argv0;
    if (Deno.mainModule?.startsWith("file:")) {
      argv[1] = pathFromURL(new URL(Deno.mainModule));
    } else {
      argv[1] = join(Deno.cwd(), "$deno$node.mjs");
    }
    for (let i = 0; i < args.length; i++) {
      argv[i + 2] = args[i];
    }
    for (const [key, value] of Object.entries(denoVersions)) {
      versions[key] = value;
    }
    core.setNextTickCallback(processTicksAndRejections);
    core.setMacrotaskCallback(runNextTicks);
    enableNextTick();

    if (!io.stdout.isTerminal()) {
      stdout = process.stdout = createWritableStdioStream(io.stdout, "stdout");
    }
    if (!io.stderr.isTerminal()) {
      stderr = process.stderr = createWritableStdioStream(io.stderr, "stderr");
    }
    arch = arch_();
    platform = isWindows ? "win32" : Deno.build.os;
    pid = Deno.pid;
    ppid = Deno.ppid;
    initializeDebugEnv(nodeDebug);
    if (getOptionValue("--warnings")) {
      process.on("warning", onWarning);
    }
    const newStdin = initStdin();
    if (newStdin) {
      stdin = process.stdin = newStdin;
    }
    delete internals.__bootstrapNodeProcess;
  } else {
    stdin = process.stdin = initStdin(true);
    stdout = process.stdout = createWritableStdioStream(
      io.stdout,
      "stdout",
      true,
    );
    stderr = process.stderr = createWritableStdioStream(
      io.stderr,
      "stderr",
      true,
    );
  }
};

export default process;
