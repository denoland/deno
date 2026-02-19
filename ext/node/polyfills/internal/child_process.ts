// Copyright 2018-2026 the Deno authors. MIT license.

// This module implements 'child_process' module of Node.JS API.
// ref: https://nodejs.org/api/child_process.html

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import { core, internals } from "ext:core/mod.js";
import {
  op_node_in_npm_package,
  op_node_ipc_buffer_constructor,
  op_node_ipc_read_advanced,
  op_node_ipc_read_json,
  op_node_ipc_ref,
  op_node_ipc_unref,
  op_node_ipc_write_advanced,
  op_node_ipc_write_json,
  op_node_parse_shell_args,
  op_node_translate_cli_args,
} from "ext:core/ops";
import {
  ArrayIsArray,
  ArrayPrototypeFilter,
  ArrayPrototypeJoin,
  ArrayPrototypePush,
  ArrayPrototypeSlice,
  ArrayPrototypeSort,
  ArrayPrototypeUnshift,
  ObjectHasOwn,
  StringPrototypeIncludes,
  StringPrototypeStartsWith,
  StringPrototypeToUpperCase,
} from "ext:deno_node/internal/primordials.mjs";
import assert from "node:assert";
import { EventEmitter } from "node:events";
import { os } from "ext:deno_node/internal_binding/constants.ts";
import { notImplemented } from "ext:deno_node/_utils.ts";
import { Readable, Stream, Writable } from "node:stream";
import { isWindows } from "ext:deno_node/_util/os.ts";
import { nextTick } from "ext:deno_node/_next_tick.ts";
import {
  AbortError,
  ERR_INVALID_ARG_TYPE,
  ERR_INVALID_ARG_VALUE,
  ERR_INVALID_SYNC_FORK_INPUT,
  ERR_IPC_CHANNEL_CLOSED,
  ERR_IPC_SYNC_FORK,
  ERR_UNKNOWN_SIGNAL,
} from "ext:deno_node/internal/errors.ts";
import { Buffer } from "node:buffer";
import { FastBuffer } from "ext:deno_node/internal/buffer.mjs";
import { errnoException } from "ext:deno_node/internal/errors.ts";
import { ErrnoException } from "ext:deno_node/_global.d.ts";
import { codeMap } from "ext:deno_node/internal_binding/uv.ts";
import {
  isInt32,
  validateBoolean,
  validateObject,
  validateOneOf,
  validateString,
} from "ext:deno_node/internal/validators.mjs";
import { kEmptyObject } from "ext:deno_node/internal/util.mjs";
import { getValidatedPath } from "ext:deno_node/internal/fs/utils.mjs";
import process from "node:process";
import { StringPrototypeSlice } from "ext:deno_node/internal/primordials.mjs";
import { StreamBase } from "ext:deno_node/internal_binding/stream_wrap.ts";
import { Pipe, socketType } from "ext:deno_node/internal_binding/pipe_wrap.ts";
import { Socket } from "node:net";
import {
  kExtraStdio,
  kInputOption,
  kIpc,
  kNeedsNpmProcessState,
  kSerialization,
} from "ext:deno_process/40_process.js";

export function mapValues<T, O>(
  record: Readonly<Record<string, T>>,
  transformer: (value: T) => O,
): Record<string, O> {
  const ret: Record<string, O> = {};
  const entries = Object.entries(record);

  for (const [key, value] of entries) {
    if (typeof value === "undefined") {
      continue;
    }
    if (value === null) {
      continue;
    }

    const mappedValue = transformer(value);

    ret[key] = mappedValue;
  }

  return ret;
}

type NodeStdio = "pipe" | "overlapped" | "ignore" | "inherit" | "ipc";
type DenoStdio = "inherit" | "piped" | "null";

export function stdioStringToArray(
  stdio: NodeStdio,
  channel: NodeStdio | number,
) {
  const options: (NodeStdio | number)[] = [];

  switch (stdio) {
    case "ignore":
    case "overlapped":
    case "pipe":
      options.push(stdio, stdio, stdio);
      break;
    case "inherit":
      options.push(stdio, stdio, stdio);
      break;
    default:
      throw new ERR_INVALID_ARG_VALUE("stdio", stdio);
  }

  if (channel) options.push(channel);

  return options;
}

const kClosesNeeded = Symbol("_closesNeeded");
const kClosesReceived = Symbol("_closesReceived");
const kCanDisconnect = Symbol("_canDisconnect");

// We only want to emit a close event for the child process when all of
// the writable streams have closed. The value of `child[kClosesNeeded]` should be 1 +
// the number of opened writable streams (note this excludes `stdin`).
function maybeClose(child: ChildProcess) {
  child[kClosesReceived]++;
  if (child[kClosesNeeded] === child[kClosesReceived]) {
    child.emit("close", child.exitCode, child.signalCode);
  }
}

function flushStdio(subprocess: ChildProcess) {
  const stdio = subprocess.stdio;

  if (stdio == null) return;

  for (let i = 0; i < stdio.length; i++) {
    const stream = stdio[i];
    if (!stream || !stream.readable) {
      continue;
    }
    stream.resume();
  }
}

// Wraps a resource in a class that implements
// StreamBase, so it can be used with node streams
class StreamResource implements StreamBase {
  #rid: number;
  #isUnref = false;
  #pendingPromises: Set<Promise<number>> = new Set();
  constructor(rid: number) {
    this.#rid = rid;
  }
  close(): void {
    core.close(this.#rid);
  }
  async read(p: Uint8Array): Promise<number | null> {
    const readPromise = core.read(this.#rid, p);
    this.#pendingPromises.add(readPromise);
    if (this.#isUnref) {
      core.unrefOpPromise(readPromise);
    }
    try {
      const nread = await readPromise;
      return nread > 0 ? nread : null;
    } finally {
      this.#pendingPromises.delete(readPromise);
    }
  }
  ref(): void {
    this.#isUnref = false;
    for (const promise of this.#pendingPromises) {
      core.refOpPromise(promise);
    }
  }
  unref(): void {
    this.#isUnref = true;
    for (const promise of this.#pendingPromises) {
      core.unrefOpPromise(promise);
    }
  }
  async write(p: Uint8Array): Promise<number> {
    const writePromise = core.write(this.#rid, p);
    this.#pendingPromises.add(writePromise);
    if (this.#isUnref) {
      core.unrefOpPromise(writePromise);
    }
    try {
      return await writePromise;
    } finally {
      this.#pendingPromises.delete(writePromise);
    }
  }
}

export class ChildProcess extends EventEmitter {
  /**
   * The exit code of the child process. This property will be `null` until the child process exits.
   */
  exitCode: number | null = null;

  /**
   * This property is set to `true` after `kill()` is called.
   */
  killed = false;

  /**
   * The PID of this child process.
   */
  pid!: number;

  /**
   * The signal received by this child process.
   */
  signalCode: string | null = null;

  /**
   * Command line arguments given to this child process.
   */
  spawnargs: string[] = [];

  /**
   * The executable file name of this child process.
   */
  spawnfile: string = "";

  /**
   * This property represents the child process's stdin.
   */
  stdin: Writable | null = null;

  /**
   * This property represents the child process's stdout.
   */
  stdout: Readable | null = null;

  /**
   * This property represents the child process's stderr.
   */
  stderr: Readable | null = null;

  /**
   * Pipes to this child process.
   */
  stdio: [Writable | null, Readable | null, Readable | null] = [
    null,
    null,
    null,
  ];

  disconnect?: () => void;

  #process!: Deno.ChildProcess;
  #spawned = Promise.withResolvers<void>();
  [kClosesNeeded] = 1;
  [kClosesReceived] = 0;
  [kCanDisconnect] = false;

  constructor() {
    super();
  }

  /**
   * Internal spawn method used by Node.js internals.
   * This is called after creating a ChildProcess instance.
   */
  spawn(options: {
    file?: string;
    args?: string[];
    cwd?: string;
    stdio?: Array<NodeStdio | number | Stream | null | undefined> | NodeStdio;
    envPairs?: string[];
    windowsVerbatimArguments?: boolean;
    detached?: boolean;
    signal?: AbortSignal;
    serialization?: "json" | "advanced";
    // deno-lint-ignore no-explicit-any
    [key: string]: any;
  }): void {
    // Validate options
    if (options == null || typeof options !== "object") {
      throw new ERR_INVALID_ARG_TYPE("options", "object", options);
    }

    // Validate envPairs before file (Node.js validation order)
    const { envPairs } = options;
    if (envPairs !== undefined && !ArrayIsArray(envPairs)) {
      throw new ERR_INVALID_ARG_TYPE("options.envPairs", "Array", envPairs);
    }

    // Validate args
    const { args } = options;
    if (args !== undefined && !ArrayIsArray(args)) {
      throw new ERR_INVALID_ARG_TYPE("options.args", "Array", args);
    }

    // Validate file
    const { file } = options;
    if (file == null || typeof file !== "string") {
      throw new ERR_INVALID_ARG_TYPE("options.file", "string", file);
    }

    this.#spawnInternal(file, args || [], options);
  }

  /**
   * Internal method that performs the actual spawning.
   */
  #spawnInternal(
    command: string,
    args: string[],
    options: {
      cwd?: string;
      stdio?: Array<NodeStdio | number | Stream | null | undefined> | NodeStdio;
      envPairs?: string[];
      windowsVerbatimArguments?: boolean;
      detached?: boolean;
      signal?: AbortSignal;
      serialization?: "json" | "advanced";
      // deno-lint-ignore no-explicit-any
      [key: string]: any;
    },
  ): void {
    const {
      stdio = ["pipe", "pipe", "pipe"],
      cwd,
      signal,
      windowsVerbatimArguments = false,
      detached,
      envPairs,
    } = options;

    // Convert envPairs array to env object
    const env: Record<string, string> = {};
    if (envPairs) {
      for (const pair of envPairs) {
        const idx = pair.indexOf("=");
        if (idx !== -1) {
          env[pair.substring(0, idx)] = pair.substring(idx + 1);
        }
      }
    }

    const serialization = options.serialization || "json";
    const normalizedStdio = normalizeStdioOption(stdio);
    const [
      stdin = "pipe",
      stdout = "pipe",
      stderr = "pipe",
      ...extraStdio
    ] = normalizedStdio;

    // buildCommand handles Node.js to Deno CLI arg translation when spawning Deno
    // Note: args[0] is argv0 (prepended by normalizeSpawnArguments), so we skip it
    const [cmd, cmdArgs, includeNpmProcessState] = buildCommand(
      command,
      args.slice(1),
      env,
    );

    this.spawnfile = cmd;
    this.spawnargs = [cmd, ...cmdArgs];

    const ipc = normalizedStdio.indexOf("ipc");

    const extraStdioOffset = 3; // stdin, stdout, stderr

    const extraStdioNormalized: DenoStdio[] = [];
    for (let i = 0; i < extraStdio.length; i++) {
      const fd = i + extraStdioOffset;
      if (fd === ipc) extraStdioNormalized.push("null");
      extraStdioNormalized.push(toDenoStdio(extraStdio[i]));
    }

    try {
      this.#process = new Deno.Command(cmd, {
        args: cmdArgs,
        clearEnv: true,
        cwd,
        env,
        stdin: toDenoStdio(stdin),
        stdout: toDenoStdio(stdout),
        stderr: toDenoStdio(stderr),
        windowsRawArguments: windowsVerbatimArguments,
        detached,
        [kSerialization]: serialization,
        [kIpc]: ipc, // internal
        [kExtraStdio]: extraStdioNormalized,
        [kNeedsNpmProcessState]: options[kNeedsNpmProcessState] ||
          includeNpmProcessState,
      }).spawn();
      this.pid = this.#process.pid;

      // Get stdio rids to create Socket instances
      const stdioRids = internals.getStdioRids(this.#process);

      if (stdin === "pipe") {
        assert(this.#process.stdin);
        if (stdioRids.stdinRid !== null) {
          // Create Socket instance for stdin (like Node.js does)
          this.stdin = new Socket({
            handle: new Pipe(
              socketType.SOCKET,
              new StreamResource(stdioRids.stdinRid),
            ),
            writable: true,
            readable: false,
          });
        } else {
          // Fallback to web stream conversion
          this.stdin = Writable.fromWeb(this.#process.stdin);
        }
      }

      if (stdin instanceof Stream) {
        this.stdin = stdin;
      }
      if (stdout instanceof Stream) {
        this.stdout = stdout;
      }
      if (stderr instanceof Stream) {
        this.stderr = stderr;
      }

      if (stdout === "pipe") {
        assert(this.#process.stdout);
        this[kClosesNeeded]++;
        if (stdioRids.stdoutRid !== null) {
          // Create Socket instance for stdout (like Node.js does)
          this.stdout = new Socket({
            handle: new Pipe(
              socketType.SOCKET,
              new StreamResource(stdioRids.stdoutRid),
            ),
            writable: false,
            readable: true,
          });
        } else {
          // Fallback to web stream conversion
          this.stdout = Readable.fromWeb(this.#process.stdout);
        }
        this.stdout.on("close", () => {
          maybeClose(this);
        });
      }

      if (stderr === "pipe") {
        assert(this.#process.stderr);
        this[kClosesNeeded]++;
        if (stdioRids.stderrRid !== null) {
          // Create Socket instance for stderr (like Node.js does)
          this.stderr = new Socket({
            handle: new Pipe(
              socketType.SOCKET,
              new StreamResource(stdioRids.stderrRid),
            ),
            writable: false,
            readable: true,
          });
        } else {
          // Fallback to web stream conversion
          this.stderr = Readable.fromWeb(this.#process.stderr);
        }
        this.stderr.on("close", () => {
          maybeClose(this);
        });
      }

      this.stdio[0] = this.stdin;
      this.stdio[1] = this.stdout;
      this.stdio[2] = this.stderr;

      if (ipc >= 0) {
        this.stdio[ipc] = null;
      }

      const pipeRids = internals.getExtraPipeRids(this.#process);
      for (let i = 0; i < pipeRids.length; i++) {
        const rid: number | null = pipeRids[i];
        const fd = i + extraStdioOffset;
        if (rid) {
          this[kClosesNeeded]++;
          this.stdio[fd] = new Socket(
            {
              handle: new Pipe(
                socketType.IPC,
                new StreamResource(rid),
              ),
              // deno-lint-ignore no-explicit-any
            } as any,
          );
          this.stdio[fd]?.on("close", () => {
            maybeClose(this);
          });
        }
      }

      nextTick(() => {
        this.emit("spawn");
        this.#spawned.resolve();
      });

      if (signal) {
        const onAbortListener = () => {
          try {
            if (this.kill("SIGKILL")) {
              this.emit("error", new AbortError());
            }
          } catch (err) {
            this.emit("error", err);
          }
        };
        if (signal.aborted) {
          nextTick(onAbortListener);
        } else {
          signal.addEventListener("abort", onAbortListener, { once: true });
          this.addListener(
            "exit",
            () => signal.removeEventListener("abort", onAbortListener),
          );
        }
      }

      const pipeRid = internals.getIpcPipeRid(this.#process);
      if (typeof pipeRid == "number") {
        setupChannel(this, pipeRid, serialization);
        this[kClosesNeeded]++;
        this.on("disconnect", () => {
          maybeClose(this);
        });
      }

      (async () => {
        const status = await this.#process.status;
        this.signalCode = this.signalCode || status.signal || null;
        if (this.signalCode) {
          this.exitCode = null;
        } else {
          this.exitCode = status.code;
        }
        this.#spawned.promise.then(async () => {
          // The 'exit' and 'close' events must be emitted after the 'spawn' event.
          this.emit("exit", this.exitCode, this.signalCode);
          await this.#_waitForChildStreamsToClose();
          this.#closePipes();
          maybeClose(this);
          nextTick(flushStdio, this);
        });
      })();
    } catch (err) {
      let e = err;
      if (e instanceof Deno.errors.NotFound) {
        // args.slice(1) to exclude argv0 (prepended by normalizeSpawnArguments)
        e = _createSpawnError("ENOENT", command, args.slice(1));
      }
      this.#_handleError(e);
    }
  }

  /**
   * @param signal NOTE: this parameter is not yet implemented.
   */
  kill(signal?: number | string): boolean {
    if (this.killed) {
      return this.killed;
    }

    const denoSignal = signal == null ? "SIGTERM" : toDenoSignal(signal);
    this.#closePipes();
    try {
      this.#process.kill(denoSignal);
    } catch (err) {
      const alreadyClosed = err instanceof TypeError ||
        err instanceof Deno.errors.PermissionDenied;
      if (!alreadyClosed) {
        throw err;
      }
    }

    /* Cancel any pending IPC I/O */
    if (this[kCanDisconnect]) {
      this.disconnect?.();
    }

    this.killed = true;
    this.signalCode = denoSignal;
    return this.killed;
  }

  ref() {
    this.#process.ref();
  }

  unref() {
    this.#process.unref();
  }

  async #_waitForChildStreamsToClose() {
    const promises = [] as Array<Promise<void>>;
    // Don't close parent process stdin if that's passed through
    if (this.stdin && !this.stdin.destroyed && this.stdin !== process.stdin) {
      assert(this.stdin);
      this.stdin.destroy();
      promises.push(waitForStreamToClose(this.stdin));
    }
    // Only readable streams need to be closed
    if (
      this.stdout && !this.stdout.destroyed && this.stdout instanceof Readable
    ) {
      promises.push(waitForReadableToClose(this.stdout));
    }
    // Only readable streams need to be closed
    if (
      this.stderr && !this.stderr.destroyed && this.stderr instanceof Readable
    ) {
      promises.push(waitForReadableToClose(this.stderr));
    }
    await Promise.all(promises);
  }

  #_handleError(err: unknown) {
    nextTick(() => {
      this.emit("error", err); // TODO(uki00a) Convert `err` into nodejs's `SystemError` class.
    });
  }

  #closePipes() {
    if (this.stdin) {
      assert(this.stdin);
      this.stdin.destroy();
    }
  }
}

const supportedNodeStdioTypes: NodeStdio[] = [
  "pipe",
  "ignore",
  "inherit",
  "ipc",
];
function toDenoStdio(
  pipe: NodeStdio | number | Stream | null | undefined,
): DenoStdio {
  if (pipe instanceof Stream) {
    return "inherit";
  }
  if (typeof pipe === "number") {
    /* Assume it's a rid returned by fs APIs */
    return pipe;
  }

  if (
    !supportedNodeStdioTypes.includes(pipe as NodeStdio)
  ) {
    notImplemented(`toDenoStdio pipe=${typeof pipe} (${pipe})`);
  }
  switch (pipe) {
    case "pipe":
    case undefined:
    case null:
      return "piped";
    case "ignore":
      return "null";
    case "inherit":
      return "inherit";
    case "ipc":
      return "ipc_for_internal_use";
    default:
      notImplemented(`toDenoStdio pipe=${typeof pipe} (${pipe})`);
  }
}

function toDenoSignal(signal: number | string): Deno.Signal {
  if (typeof signal === "number") {
    for (const name of keys(os.signals)) {
      if (os.signals[name] === signal) {
        return name as Deno.Signal;
      }
    }
    throw new ERR_UNKNOWN_SIGNAL(String(signal));
  }

  const denoSignal = signal as Deno.Signal;
  if (denoSignal in os.signals) {
    return denoSignal;
  }
  throw new ERR_UNKNOWN_SIGNAL(signal);
}

function keys<T extends Record<string, unknown>>(object: T): Array<keyof T> {
  return Object.keys(object);
}

export interface ChildProcessOptions {
  /**
   * Current working directory of the child process.
   */
  cwd?: string | URL;

  /**
   * Environment variables passed to the child process.
   */
  env?: Record<string, string | number | boolean>;

  /**
   * This option defines child process's stdio configuration.
   * @see https://nodejs.org/api/child_process.html#child_process_options_stdio
   */
  stdio?: Array<NodeStdio | number | Stream | null | undefined> | NodeStdio;

  /**
   * Whether to spawn the process in a detached state.
   */
  detached?: boolean;

  /**
   * NOTE: This option is not yet implemented.
   */
  uid?: number;

  /**
   * NOTE: This option is not yet implemented.
   */
  gid?: number;

  /**
   * NOTE: This option is not yet implemented.
   */
  argv0?: string;

  /**
   * * If this option is `true`, run the command in the shell.
   * * If this option is a string, run the command in the specified shell.
   */
  shell?: string | boolean;

  /**
   * Allows aborting the child process using an AbortSignal.
   */
  signal?: AbortSignal;

  /**
   * NOTE: This option is not yet implemented.
   */
  serialization?: "json" | "advanced";

  /** No quoting or escaping of arguments is done on Windows. Ignored on Unix.
   * Default: false. */
  windowsVerbatimArguments?: boolean;

  /**
   * NOTE: This option is not yet implemented.
   */
  windowsHide?: boolean;
}

function copyProcessEnvToEnv(
  env: Record<string, string | number | boolean | undefined>,
  name: string,
  optionEnv?: Record<string, string | number | boolean>,
) {
  if (
    Deno.env.get(name) &&
    (!optionEnv ||
      !ObjectHasOwn(optionEnv, name))
  ) {
    env[name] = Deno.env.get(name);
  }
}

function normalizeStdioOption(
  stdio: Array<NodeStdio | number | null | undefined | Stream> | NodeStdio = [
    "pipe",
    "pipe",
    "pipe",
  ],
): [
  Stream | NodeStdio | number,
  Stream | NodeStdio | number,
  Stream | NodeStdio | number,
  ...Array<Stream | NodeStdio | number>,
] {
  if (Array.isArray(stdio)) {
    // `[0, 1, 2]` is equivalent to `"inherit"`
    if (
      stdio.length === 3 &&
      (stdio[0] === 0 && stdio[1] === 1 && stdio[2] === 2)
    ) {
      return ["inherit", "inherit", "inherit"];
    }

    // `[null, null, null]` is equivalent to `"pipe"
    if (
      stdio.length === 3 &&
        stdio[0] === null || stdio[1] === null || stdio[2] === null
    ) {
      return ["pipe", "pipe", "pipe"];
    }

    // At least 3 stdio must be created to match node
    while (stdio.length < 3) {
      ArrayPrototypePush(stdio, undefined);
    }
    return stdio;
  } else {
    switch (stdio) {
      case "overlapped":
        if (isWindows) {
          notImplemented("normalizeStdioOption overlapped (on windows)");
        }
        // 'overlapped' is same as 'piped' on non Windows system.
        return ["pipe", "pipe", "pipe"];
      case "pipe":
        return ["pipe", "pipe", "pipe"];
      case "inherit":
        return ["inherit", "inherit", "inherit"];
      case "ignore":
        return ["ignore", "ignore", "ignore"];
      default:
        notImplemented(`normalizeStdioOption stdio=${typeof stdio} (${stdio})`);
    }
  }
}

// Valid stdio string values
const validStdioStrings = ["ignore", "pipe", "inherit", "overlapped"];

// Result type for getValidStdio
export interface StdioResult {
  stdio: Array<{ type: string; fd?: number } | null>;
  ipc: number | undefined;
  ipcFd: number | undefined;
}

/**
 * Validates and processes stdio configuration.
 * This is an internal function used by Node.js's child_process module.
 */
export function getValidStdio(
  // deno-lint-ignore no-explicit-any
  stdio: any,
  sync?: boolean,
): StdioResult {
  let ipc: number | undefined;
  let ipcFd: number | undefined;

  // If stdio is a string, validate it
  if (typeof stdio === "string") {
    if (!validStdioStrings.includes(stdio)) {
      throw new ERR_INVALID_ARG_VALUE("stdio", stdio);
    }
    // Convert string to array
    stdio = [stdio, stdio, stdio];
  } else if (!ArrayIsArray(stdio)) {
    throw new ERR_INVALID_ARG_VALUE("stdio", stdio);
  }

  // Expand stdio array to at least 3 elements (mutates the input array)
  while (stdio.length < 3) {
    ArrayPrototypePush(stdio, undefined);
  }

  // Process each stdio element
  const result: Array<{ type: string; fd?: number } | null> = [];

  for (let i = 0; i < stdio.length; i++) {
    const value = stdio[i];

    if (value === "ipc") {
      if (sync) {
        throw new ERR_IPC_SYNC_FORK();
      }
      ipc = i;
      ipcFd = i;
      result.push({ type: "ipc" });
    } else if (value === "ignore" || value === null) {
      result.push({ type: "ignore" });
    } else if (value === "pipe" || value === undefined) {
      result.push({ type: "pipe" });
    } else if (value === "inherit") {
      result.push({ type: "inherit" });
    } else if (value === "overlapped") {
      result.push({ type: "overlapped" });
    } else if (typeof value === "number") {
      result.push({ type: "fd", fd: value });
    } else if (typeof value === "string") {
      // Invalid string value
      throw new ERR_INVALID_SYNC_FORK_INPUT(value);
    } else if (typeof value === "object" && value !== null) {
      // Check if it's a Stream with fd property (like process.stdin/stdout/stderr)
      if (
        value.fd !== undefined && typeof value.fd === "number"
      ) {
        result.push({ type: "fd", fd: value.fd });
      } else if (value instanceof Stream) {
        // Valid Stream object but without fd
        result.push({ type: "pipe" });
      } else {
        // Invalid object
        throw new ERR_INVALID_ARG_VALUE("stdio", value);
      }
    } else {
      throw new ERR_INVALID_ARG_VALUE("stdio", value);
    }
  }

  return {
    stdio: result,
    ipc,
    ipcFd,
  };
}

// Check for null bytes in a string and throw ERR_INVALID_ARG_VALUE if found
export function validateNullByteNotInArg(value: string, name: string): void {
  if (StringPrototypeIncludes(value, "\0")) {
    throw new ERR_INVALID_ARG_VALUE(
      name,
      value,
      "must be a string without null bytes",
    );
  }
}

export function normalizeSpawnArguments(
  file: string,
  args: string[],
  options: SpawnOptions & SpawnSyncOptions,
) {
  validateString(file, "file");

  if (file.length === 0) {
    throw new ERR_INVALID_ARG_VALUE("file", file, "cannot be empty");
  }

  // Check for null bytes in file
  validateNullByteNotInArg(file, "file");

  if (ArrayIsArray(args)) {
    args = ArrayPrototypeSlice(args);
  } else if (args == null) {
    args = [];
  } else if (typeof args !== "object") {
    throw new ERR_INVALID_ARG_TYPE("args", "object", args);
  } else {
    options = args;
    args = [];
  }

  // Check for null bytes in args
  for (let i = 0; i < args.length; i++) {
    const arg = args[i];
    if (typeof arg === "string") {
      validateNullByteNotInArg(arg, `args[${i}]`);
    }
  }

  if (options === undefined) {
    options = kEmptyObject;
  } else {
    validateObject(options, "options");
  }

  let cwd = options.cwd;

  // Validate the cwd, if present.
  if (cwd != null) {
    cwd = getValidatedPath(cwd, "options.cwd") as string;
    validateNullByteNotInArg(cwd, "options.cwd");
  }

  // Validate detached, if present.
  if (options.detached != null) {
    validateBoolean(options.detached, "options.detached");
  }

  // Validate the uid, if present.
  if (options.uid != null && !isInt32(options.uid)) {
    throw new ERR_INVALID_ARG_TYPE("options.uid", "int32", options.uid);
  }

  // Validate the gid, if present.
  if (options.gid != null && !isInt32(options.gid)) {
    throw new ERR_INVALID_ARG_TYPE("options.gid", "int32", options.gid);
  }

  // Validate the shell, if present.
  if (
    options.shell != null &&
    typeof options.shell !== "boolean" &&
    typeof options.shell !== "string"
  ) {
    throw new ERR_INVALID_ARG_TYPE(
      "options.shell",
      ["boolean", "string"],
      options.shell,
    );
  }
  if (typeof options.shell === "string") {
    validateNullByteNotInArg(options.shell, "options.shell");
  }

  // Validate argv0, if present.
  if (options.argv0 != null) {
    validateString(options.argv0, "options.argv0");
    validateNullByteNotInArg(options.argv0, "options.argv0");
  }

  // Validate windowsHide, if present.
  if (options.windowsHide != null) {
    validateBoolean(options.windowsHide, "options.windowsHide");
  }

  // Validate windowsVerbatimArguments, if present.
  let { windowsVerbatimArguments } = options;
  if (windowsVerbatimArguments != null) {
    validateBoolean(
      windowsVerbatimArguments,
      "options.windowsVerbatimArguments",
    );
  }

  validateOneOf(options.serialization, "options.serialization", [
    undefined,
    "json",
    "advanced",
  ]);

  if (options.shell) {
    // When args are provided, escape them to prevent shell injection.
    // When no args are provided (just a string command), the user intends
    // for shell interpretation, so don't escape.
    let command;
    if (args.length > 0) {
      const escapedParts = [escapeShellArg(file), ...args.map(escapeShellArg)];
      command = ArrayPrototypeJoin(escapedParts, " ");
    } else {
      command = file;
    }
    // Set the shell, switches, and commands.
    // Note: transformDenoShellCommand is NOT called here because buildCommand()
    // already handles it for both `-c` (POSIX) and `/d /s /c` (cmd.exe) cases.
    // Calling it here would cause double transformation.
    if (process.platform === "win32") {
      if (typeof options.shell === "string") {
        file = options.shell;
      } else {
        file = Deno.env.get("comspec") || "cmd.exe";
      }
      // '/d /s /c' is used only for cmd.exe.
      if (/^(?:.*\\)?cmd(?:\.exe)?$/i.exec(file) !== null) {
        args = ["/d", "/s", "/c", `"${command}"`];
        windowsVerbatimArguments = true;
      } else {
        args = ["-c", command];
      }
    } else {
      /** TODO: add Android condition */
      if (typeof options.shell === "string") {
        file = options.shell;
      } else {
        file = "/bin/sh";
      }
      args = ["-c", command];
    }
  }

  if (typeof options.argv0 === "string") {
    ArrayPrototypeUnshift(args, options.argv0);
  } else {
    ArrayPrototypeUnshift(args, file);
  }

  const env = options.env || Deno.env.toObject();
  const envPairs: string[][] = [];

  // process.env.NODE_V8_COVERAGE always propagates, making it possible to
  // collect coverage for programs that spawn with white-listed environment.
  copyProcessEnvToEnv(env, "NODE_V8_COVERAGE", options.env);

  /** TODO: add `isZOS` condition */

  let envKeys: string[] = [];
  // Prototype values are intentionally included.
  for (const key in env) {
    if (Object.hasOwn(env, key)) {
      ArrayPrototypePush(envKeys, key);
    }
  }

  if (process.platform === "win32") {
    // On Windows env keys are case insensitive. Filter out duplicates,
    // keeping only the first one (in lexicographic order)
    /** TODO: implement SafeSet and makeSafe */
    const sawKey = new Set();
    envKeys = ArrayPrototypeFilter(
      ArrayPrototypeSort(envKeys),
      (key: string) => {
        const uppercaseKey = StringPrototypeToUpperCase(key);
        if (sawKey.has(uppercaseKey)) {
          return false;
        }
        sawKey.add(uppercaseKey);
        return true;
      },
    );
  }

  for (const key of envKeys) {
    const value = env[key];
    if (value !== undefined) {
      // Check for null bytes in env keys and values
      validateNullByteNotInArg(key, `options.env['${key}']`);
      validateNullByteNotInArg(String(value), `options.env['${key}']`);
      ArrayPrototypePush(envPairs, `${key}=${value}`);
    }
  }

  return {
    // Make a shallow copy so we don't clobber the user's options object.
    ...options,
    args,
    cwd,
    detached: !!options.detached,
    env,
    envPairs,
    file,
    windowsHide: !!options.windowsHide,
    windowsVerbatimArguments: !!windowsVerbatimArguments,
    serialization: options.serialization || "json",
  };
}

function waitForReadableToClose(readable: Readable) {
  readable.resume(); // Ensure buffered data will be consumed.
  return waitForStreamToClose(readable as unknown as Stream);
}

function waitForStreamToClose(stream: Stream) {
  const deferred = Promise.withResolvers<void>();
  const cleanup = () => {
    stream.removeListener("close", onClose);
    stream.removeListener("error", onError);
  };
  const onClose = () => {
    cleanup();
    deferred.resolve();
  };
  const onError = (err: Error) => {
    cleanup();
    deferred.reject(err);
  };
  stream.once("close", onClose);
  stream.once("error", onError);
  return deferred.promise;
}

/**
 * Escapes a string for safe use as a shell argument.
 * On Unix, wraps in single quotes and escapes embedded single quotes.
 * On Windows, wraps in double quotes and escapes embedded double quotes and backslashes.
 */
function escapeShellArg(arg: string): string {
  if (process.platform === "win32") {
    // Windows: use double quotes, escape double quotes and backslashes
    // Empty string needs to be quoted
    if (arg === "") {
      return '""';
    }
    // If no special characters, return as-is
    if (!/[\s"\\]/.test(arg)) {
      return arg;
    }
    // Escape backslashes before quotes, then escape quotes
    let escaped = arg.replace(/(\\*)"/g, '$1$1\\"');
    // Escape trailing backslashes
    escaped = escaped.replace(/(\\+)$/, "$1$1");
    return `"${escaped}"`;
  } else {
    // Unix: use single quotes, escape embedded single quotes
    // Empty string needs to be quoted
    if (arg === "") {
      return "''";
    }
    // If no special characters, return as-is
    if (!/[^a-zA-Z0-9_./-]/.test(arg)) {
      return arg;
    }
    // Wrap in single quotes and escape any embedded single quotes
    // Single quotes are escaped by ending the string, adding an escaped quote, and starting a new string
    return "'" + arg.replace(/'/g, "'\\''") + "'";
  }
}

/**
 * Transforms a shell command that invokes Deno with Node.js flags into Deno-compatible flags.
 * Uses the Rust CLI parser (op_node_translate_cli_args) to handle argument translation,
 * including subcommand detection, -c/--check flag handling, and adding "run -A".
 */
function transformDenoShellCommand(
  command: string,
  env?: Record<string, string | number | boolean>,
  isCmdExe: boolean = false,
): string {
  const denoPath = Deno.execPath();

  // Check if the command starts with the Deno executable (possibly quoted)
  const quotedDenoPath = `"${denoPath}"`;
  const singleQuotedDenoPath = `'${denoPath}'`;

  let startsWithDeno = false;
  let denoPathLength = 0;
  let shellVarPrefix = "";

  if (command.startsWith(quotedDenoPath)) {
    startsWithDeno = true;
    denoPathLength = quotedDenoPath.length;
  } else if (command.startsWith(singleQuotedDenoPath)) {
    startsWithDeno = true;
    denoPathLength = singleQuotedDenoPath.length;
  } else if (command.startsWith(denoPath)) {
    startsWithDeno = true;
    denoPathLength = denoPath.length;
  } else if (env) {
    // Check for shell variable that references the Deno path
    // Pattern: "${VARNAME}", "$VARNAME", ${VARNAME}, or $VARNAME at start of command
    const shellVarMatch = command.match(
      /^(?:"\$\{([^}]+)\}"|\"\$([A-Za-z_][A-Za-z0-9_]*)\"|\$\{([^}]+)\}|\$([A-Za-z_][A-Za-z0-9_]*))/,
    );
    if (shellVarMatch) {
      const varName = shellVarMatch[1] || shellVarMatch[2] ||
        shellVarMatch[3] || shellVarMatch[4];
      const varValue = env[varName];
      if (varValue !== undefined && String(varValue) === denoPath) {
        startsWithDeno = true;
        shellVarPrefix = shellVarMatch[0];
        denoPathLength = shellVarMatch[0].length;
      }
    }
  }

  if (!startsWithDeno) {
    return command;
  }

  // Extract the rest of the command after the Deno path
  const rest = command.slice(denoPathLength).trimStart();

  if (rest.length === 0) {
    return command;
  }

  // Parse the command using the shell parser to separate arguments from
  // shell operators (redirections, pipes, etc.).
  const { args, shell_suffix: shellSuffix } = op_node_parse_shell_args(
    rest,
    isCmdExe,
  );

  try {
    const result = op_node_translate_cli_args(args, false, false);
    // Shell-quote translated args that contain metacharacters so they are
    // safe to embed in a shell command string.
    const quotedArgs = isWindows
      ? result.deno_args.map((a) => {
        // Windows cmd.exe: use double quotes for args with spaces or
        // special chars. Backslash is a path separator, not an escape.
        if (/[\s"&|<>^]/.test(a)) {
          let escaped = a.replace(/(\\*)"/g, '$1$1\\"');
          escaped = escaped.replace(/(\\+)$/, "$1$1");
          return `"${escaped}"`;
        }
        return a;
      })
      : result.deno_args.map((a) => {
        // POSIX: args with shell variable refs use double quotes to
        // preserve variable expansion. Other metacharacters use single
        // quotes.
        if (/\$\{[^}]+\}|\$[A-Za-z_]/.test(a)) {
          return '"' + a.replace(/"/g, '\\"') + '"';
        }
        if (/[();&|<>`!\n\r\s"'\\$]/.test(a)) {
          return "'" + a.replace(/'/g, "'\\''") + "'";
        }
        return a;
      });
    const prefix = shellVarPrefix || command.slice(0, denoPathLength);
    let transformed = prefix + " " + quotedArgs.join(" ");
    if (shellSuffix) {
      transformed += " " + shellSuffix;
    }

    // If the shell suffix starts with a pipe, the command after the pipe
    // may also be a Deno invocation that needs transformation.
    if (env) {
      const pipeMatch = shellSuffix.match(/^\s*\|\s*/);
      if (pipeMatch) {
        const afterPipe = shellSuffix.slice(pipeMatch[0].length);
        const transformedAfter = transformDenoShellCommand(
          afterPipe,
          env,
          isCmdExe,
        );
        if (transformedAfter !== afterPipe) {
          transformed = prefix + " " + quotedArgs.join(" ") +
            " | " + transformedAfter;
        }
      }
    }
    return transformed;
  } catch {
    // If the Rust parser fails (unknown flags), return the original command
    return command;
  }
}

/**
 * Find the first non-flag argument in a list of command line arguments.
 * This is used to determine if the user is spawning a Deno subcommand
 * or a script, and to check if the script is in an npm package.
 */
function findFirstNonFlagArg(args: string[]): string | null {
  for (let i = 0; i < args.length; i++) {
    const arg = args[i];
    // Stop at '--' - everything after is positional
    if (arg === "--") {
      return i + 1 < args.length ? args[i + 1] : null;
    }
    // If it doesn't start with '-', it's a positional argument
    if (!arg.startsWith("-")) {
      return arg;
    }
    // Skip known flags that take a value
    if (
      arg === "-e" || arg === "--eval" ||
      arg === "-p" || arg === "--print" ||
      arg === "-r" || arg === "--require" ||
      arg === "-C" || arg === "--conditions" ||
      arg === "-c" || arg === "--check" ||
      arg === "--import" ||
      arg === "--loader" ||
      arg === "--experimental-loader"
    ) {
      i++; // Skip the next arg (the value)
    }
  }
  return null;
}

/**
 * This function is based on https://github.com/nodejs/node/blob/fc6426ccc4b4cb73076356fb6dbf46a28953af01/lib/child_process.js#L504-L528.
 * Copyright Joyent, Inc. and other Node contributors. All rights reserved. MIT license.
 */
function buildCommand(
  file: string,
  args: string[],
  env: Record<string, string | number | boolean>,
): [string, string[], boolean] {
  let includeNpmProcessState = false;
  if (file === Deno.execPath()) {
    // Ensure all args are strings (Node allows numbers in args array)
    args = args.map((arg) => String(arg));

    // Find script path to check if it's in an npm package
    const firstNonFlagArg = findFirstNonFlagArg(args);
    const scriptInNpmPackage = firstNonFlagArg !== null
      ? op_node_in_npm_package(firstNonFlagArg)
      : false;

    // Use the Rust parser to translate Node.js args to Deno args
    // The parser handles Deno-style args (e.g., "run -A script.js") by passing them through unchanged
    const result = op_node_translate_cli_args(args, scriptInNpmPackage, true);
    args = result.deno_args;
    includeNpmProcessState = result.needs_npm_process_state;

    // Update NODE_OPTIONS if needed
    if (result.node_options.length > 0) {
      const options = result.node_options.join(" ");
      if (env.NODE_OPTIONS) {
        env.NODE_OPTIONS += " " + options;
      } else {
        env.NODE_OPTIONS = options;
      }
    }
  }

  // When spawning a shell with `-c` (e.g. spawn('/bin/sh', ['-c', cmd])),
  // transform any Deno commands inside the shell command string so that
  // Node.js flags are translated and `-A` is added.
  if (
    file !== Deno.execPath() &&
    args.length >= 2 &&
    args[0] === "-c"
  ) {
    const transformed = transformDenoShellCommand(args[1], env, false);
    if (transformed !== args[1]) {
      args = args.slice();
      args[1] = transformed;
    }
  }

  // Windows cmd.exe: args are ["/d", "/s", "/c", '"command"']
  if (
    file !== Deno.execPath() &&
    args.length >= 4 &&
    args[0] === "/d" && args[1] === "/s" && args[2] === "/c"
  ) {
    let cmdStr = args[3];
    // Remove wrapping quotes added by normalizeSpawnArguments
    const hasWrappingQuotes = cmdStr.startsWith('"') && cmdStr.endsWith('"');
    if (hasWrappingQuotes) {
      cmdStr = cmdStr.slice(1, -1);
    }
    const transformed = transformDenoShellCommand(cmdStr, env, true);
    if (transformed !== cmdStr) {
      args = args.slice();
      args[3] = hasWrappingQuotes ? `"${transformed}"` : transformed;
    }
  }

  return [file, args, includeNpmProcessState];
}

function _createSpawnError(
  status: string,
  command: string,
  args: string[] = [],
  sync: boolean = false,
): ErrnoException {
  const syscall = sync ? "spawnSync " : "spawn ";
  const error = errnoException(
    codeMap.get(status),
    syscall + command,
  );
  error.path = command;
  error.spawnargs = args;
  return error;
}

export interface SpawnOptions extends ChildProcessOptions {
  /**
   * NOTE: This option is not yet implemented.
   */
  timeout?: number;
  /**
   * NOTE: This option is not yet implemented.
   */
  killSignal?: string;
}

export interface SpawnSyncOptions extends
  Pick<
    ChildProcessOptions,
    | "cwd"
    | "env"
    | "argv0"
    | "stdio"
    | "uid"
    | "gid"
    | "shell"
    | "windowsVerbatimArguments"
    | "windowsHide"
  > {
  input?: string | Buffer | DataView;
  timeout?: number;
  maxBuffer?: number;
  encoding?: string;
  /**
   * NOTE: This option is not yet implemented.
   */
  killSignal?: string;
}

export interface SpawnSyncResult {
  pid?: number;
  output?: [string | null, string | Buffer | null, string | Buffer | null];
  stdout?: Buffer | string | null;
  stderr?: Buffer | string | null;
  status?: number | null;
  signal?: string | null;
  error?: Error;
}

function parseSpawnSyncOutputStreams(
  output: Deno.CommandOutput,
  name: "stdout" | "stderr",
): string | Buffer | null {
  // new Deno.Command().outputSync() returns getters for stdout and stderr that throw when set
  // to 'inherit'.
  try {
    return Buffer.from(output[name]) as string | Buffer;
  } catch {
    return null;
  }
}

function normalizeInput(input: unknown) {
  if (input == null) {
    return null;
  }
  if (typeof input === "string") {
    return Buffer.from(input);
  }
  if (input instanceof Uint8Array) {
    return input;
  }
  if (input instanceof DataView) {
    return Buffer.from(input.buffer, input.byteOffset, input.byteLength);
  }
  throw new ERR_INVALID_ARG_TYPE("input", [
    "string",
    "Buffer",
    "TypedArray",
    "DataView",
  ], input);
}

export function spawnSync(
  command: string,
  args: string[],
  options: SpawnSyncOptions,
): SpawnSyncResult {
  const {
    env = Deno.env.toObject(),
    input,
    stdio = ["pipe", "pipe", "pipe"],
    cwd,
    encoding,
    uid,
    gid,
    maxBuffer,
    windowsVerbatimArguments = false,
  } = options;
  const [
    stdin_ = "pipe",
    stdout_ = "pipe",
    stderr_ = "pipe",
    _channel, // TODO(kt3k): handle this correctly
  ] = normalizeStdioOption(stdio);
  let includeNpmProcessState = false;
  // Skip argv0 when calling buildCommand (same as #spawnInternal)
  const argsToProcess = args && args.length > 0 ? args.slice(1) : [];
  [command, args, includeNpmProcessState] = buildCommand(
    command,
    argsToProcess,
    env,
  );
  const input_ = normalizeInput(input);

  const result: SpawnSyncResult = {};
  try {
    const output = new Deno.Command(command, {
      args,
      cwd,
      env: mapValues(env, (value) => value.toString()),
      stdout: toDenoStdio(stdout_),
      stderr: toDenoStdio(stderr_),
      stdin: stdin_ == "inherit" ? "inherit" : "null",
      uid,
      gid,
      windowsRawArguments: windowsVerbatimArguments,
      [kInputOption]: input_,
      // deno-lint-ignore no-explicit-any
      [kNeedsNpmProcessState]: (options as any)[kNeedsNpmProcessState] ||
        includeNpmProcessState,
    }).outputSync();

    const status = output.signal ? null : output.code;
    let stdout = parseSpawnSyncOutputStreams(output, "stdout");
    let stderr = parseSpawnSyncOutputStreams(output, "stderr");

    if (
      (stdout && stdout.length > maxBuffer!) ||
      (stderr && stderr.length > maxBuffer!)
    ) {
      result.error = _createSpawnError("ENOBUFS", command, args, true);
    }

    if (encoding && encoding !== "buffer") {
      stdout = stdout && stdout.toString(encoding);
      stderr = stderr && stderr.toString(encoding);
    }

    result.status = status;
    result.signal = output.signal;
    result.stdout = stdout;
    result.stderr = stderr;
    result.output = [output.signal, stdout, stderr];
  } catch (err) {
    if (err instanceof Deno.errors.NotFound) {
      result.error = _createSpawnError("ENOENT", command, args, true);
    }
  }
  return result;
}

const kControlDisconnect = Symbol("kControlDisconnect");
const kPendingMessages = Symbol("kPendingMessages");

// controls refcounting for the IPC channel
class Control extends EventEmitter {
  #channel: number;
  #refs: number = 0;
  #refExplicitlySet = false;
  #connected = true;
  [kPendingMessages] = [];
  #serialization: "json" | "advanced";
  constructor(channel: number, serialization: "json" | "advanced") {
    super();
    this.#channel = channel;
    this.#serialization = serialization;
  }

  #ref() {
    if (this.#connected) {
      op_node_ipc_ref(this.#channel, this.#serialization === "json");
    }
  }

  #unref() {
    if (this.#connected) {
      op_node_ipc_unref(this.#channel, this.#serialization === "json");
    }
  }

  [kControlDisconnect]() {
    this.#unref();
    this.#connected = false;
  }

  refCounted() {
    if (++this.#refs === 1 && !this.#refExplicitlySet) {
      this.#ref();
    }
  }

  unrefCounted() {
    if (--this.#refs === 0 && !this.#refExplicitlySet) {
      this.#unref();
      this.emit("unref");
    }
  }

  ref() {
    this.#refExplicitlySet = true;
    this.#ref();
  }

  unref() {
    this.#refExplicitlySet = false;
    this.#unref();
  }
}

type InternalMessage = {
  cmd: `NODE_${string}`;
};

// deno-lint-ignore no-explicit-any
function isInternal(msg: any): msg is InternalMessage {
  if (msg && typeof msg === "object") {
    const cmd = msg["cmd"];
    if (typeof cmd === "string") {
      return StringPrototypeStartsWith(cmd, "NODE_");
    }
  }
  return false;
}

function internalCmdName(msg: InternalMessage): string {
  return StringPrototypeSlice(msg.cmd, 5);
}

let hasSetBufferConstructor = false;

export function setupChannel(
  // deno-lint-ignore no-explicit-any
  target: any,
  ipc: number,
  serialization: "json" | "advanced",
) {
  const control = new Control(ipc, serialization);
  target.channel = control;

  if (!hasSetBufferConstructor) {
    op_node_ipc_buffer_constructor(Buffer, FastBuffer.prototype);
    hasSetBufferConstructor = true;
  }

  const writeFn = serialization === "json"
    ? op_node_ipc_write_json
    : op_node_ipc_write_advanced;
  const readFn = serialization === "json"
    ? op_node_ipc_read_json
    : op_node_ipc_read_advanced;

  async function readLoop() {
    try {
      while (true) {
        if (!target.connected || target.killed) {
          return;
        }
        // TODO(nathanwhit): maybe allow returning multiple messages in a single read? needs benchmarking.
        const prom = readFn(ipc);
        // there will always be a pending read promise,
        // but it shouldn't keep the event loop from exiting
        core.unrefOpPromise(prom);
        const msg = await prom;
        if (isInternal(msg)) {
          const cmd = internalCmdName(msg);
          if (cmd === "CLOSE") {
            // Channel closed.
            target.disconnect();
            return;
          } else {
            // TODO(nathanwhit): once we add support for sending
            // handles, if we want to support deno-node IPC interop,
            // we'll need to handle the NODE_HANDLE_* messages here.
            continue;
          }
        }

        nextTick(handleMessage, msg);
      }
    } catch (err) {
      if (
        err instanceof Deno.errors.Interrupted ||
        err instanceof Deno.errors.BadResource
      ) {
        return;
      }
    }
  }

  function handleMessage(msg) {
    if (!target.channel) {
      return;
    }
    if (target.listenerCount("message") !== 0) {
      target.emit("message", msg);
      return;
    }

    ArrayPrototypePush(target.channel[kPendingMessages], msg);
  }

  target.on("newListener", () => {
    nextTick(() => {
      if (!target.channel || !target.listenerCount("message")) {
        return;
      }
      for (const msg of target.channel[kPendingMessages]) {
        target.emit("message", msg);
      }
      target.channel[kPendingMessages] = [];
    });
  });

  target.send = function (message, handle, options, callback) {
    if (typeof handle === "function") {
      callback = handle;
      handle = undefined;
      options = undefined;
    } else if (typeof options === "function") {
      callback = options;
      options = undefined;
    } else if (options !== undefined) {
      validateObject(options, "options");
    }

    options = { swallowErrors: false, ...options };

    if (message === undefined) {
      throw new TypeError("ERR_MISSING_ARGS", "message");
    }

    if (handle !== undefined) {
      notImplemented("ChildProcess.send with handle");
    }

    if (!target.connected) {
      const err = new ERR_IPC_CHANNEL_CLOSED();
      if (typeof callback === "function") {
        nextTick(callback, err);
      } else {
        nextTick(() => target.emit("error", err));
      }
      return false;
    }

    // signals whether the queue is within the limit.
    // if false, the sender should slow down.
    // this acts as a backpressure mechanism.
    const queueOk = [true];
    control.refCounted();
    writeFn(ipc, message, queueOk)
      .then(() => {
        control.unrefCounted();
        if (callback) {
          nextTick(callback, null);
        }
      }, (err: Error) => {
        control.unrefCounted();
        if (err instanceof Deno.errors.Interrupted) {
          // Channel closed on us mid-write.
        } else {
          if (typeof callback === "function") {
            nextTick(callback, err);
          } else {
            nextTick(() => target.emit("error", err));
          }
        }
      });
    return queueOk[0];
  };

  target.connected = true;

  target.disconnect = function () {
    if (!target.connected) {
      target.emit("error", new Error("IPC channel is already disconnected"));
      return;
    }

    target.connected = false;
    target[kCanDisconnect] = false;
    control[kControlDisconnect]();
    nextTick(() => {
      target.channel = null;
      core.close(ipc);
      target.emit("disconnect");
    });
  };
  target[kCanDisconnect] = true;

  // Start reading messages from the channel.
  readLoop();

  return control;
}

export default {
  ChildProcess,
  getValidStdio,
  normalizeSpawnArguments,
  stdioStringToArray,
  spawnSync,
  setupChannel,
};
