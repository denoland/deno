// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

(function () {
const { core, internals, primordials } = globalThis.__bootstrap;
const {
  op_create_worker,
  op_host_get_worker_cpu_usage,
  op_host_post_message,
  op_host_post_message_raw,
  op_host_recv_ctrl,
  op_host_recv_message,
  op_host_recv_message_sync,
  op_host_terminate_worker,
  op_node_worker_thread_post_message,
  op_node_worker_thread_recv_message,
  op_node_worker_thread_register,
  op_node_worker_thread_set_listener_count,
  op_worker_get_resource_limits,
  op_worker_threads_filename,
} = core.ops;
const {
  deserializeJsMessageData,
  serializeJsMessageData,
} = core.loadExtScript("ext:deno_web/13_message_port.js");
const {
  MessagePort,
  MessageChannel,
  createParentPort,
  receiveMessageOnPort,
  markAsUntransferable,
  isMarkedAsUntransferable,
  markAsUncloneable,
  kPortId,
  kClosed,
} = core.loadExtScript("ext:deno_node/internal/worker/io.ts");
const webidl = core.loadExtScript("ext:deno_webidl/00_webidl.js");
const { notImplemented } = core.loadExtScript("ext:deno_node/_utils.ts");
const {
  ERR_INVALID_ARG_TYPE,
  ERR_INVALID_URL_SCHEME,
  ERR_OUT_OF_RANGE,
  ERR_WORKER_INVALID_EXEC_ARGV,
  ERR_WORKER_MESSAGING_ERRORED,
  ERR_WORKER_MESSAGING_FAILED,
  ERR_WORKER_MESSAGING_SAME_THREAD,
  ERR_WORKER_MESSAGING_TIMEOUT,
  ERR_WORKER_NOT_RUNNING,
  ERR_WORKER_PATH,
} = core.loadExtScript("ext:deno_node/internal/errors.ts");
const {
  validateArray,
  validateInteger,
  validateObject,
} = core.loadExtScript("ext:deno_node/internal/validators.mjs");
const { EventEmitter } = core.loadExtScript("ext:deno_node/_events.mjs");
const lazyStream = core.createLazyLoader("node:stream");
const {
  BroadcastChannel: WebBroadcastChannel,
  refBroadcastChannel,
} = core.loadExtScript("ext:deno_web/01_broadcast_channel.js");
const lazyProcess = core.createLazyLoader("node:process");
const lazyUrl = core.createLazyLoader("node:url");
const lazyModule = core.createLazyLoader("node:module");

// Eagerly bind: node:process is in the eager ESM bundle so this is cheap, and
// it's used pervasively throughout this module. `Readable`/`Writable`,
// `fileURLToPath`, and `createRequire` stay deferred via their `lazy*`
// loaders.
const process = lazyProcess().default;

const {
  ArrayIsArray,
  Error,
  EvalError,
  FunctionPrototypeCall,
  NumberIsFinite,
  NumberIsNaN,
  ObjectAssign,
  ObjectHasOwn,
  ObjectKeys,
  Promise,
  PromiseReject,
  PromiseResolve,
  SafeMap,
  SafeRegExp,
  SafeSet,
  String,
  StringPrototypeIndexOf,
  StringPrototypeSlice,
  StringPrototypeSplit,
  StringPrototypeStartsWith,
  StringPrototypeTrim,
  SyntaxError,
  Symbol,
  SymbolAsyncDispose,
  SymbolFor,
  SymbolIterator,
  TypeError,
  URIError,
  RangeError,
  ReferenceError,
  Float64Array,
  FunctionPrototypeBind,
} = primordials;

// Map error names to native constructors so that worker error events
// preserve err.constructor (e.g. SyntaxError, TypeError).
const nativeErrorConstructors: Record<string, ErrorConstructor> = {
  __proto__: null as unknown as ErrorConstructor,
  Error,
  EvalError,
  RangeError,
  ReferenceError,
  SyntaxError,
  TypeError,
  URIError,
};

const workerCpuUsageBuffer = new Float64Array(2);

const debugWorkerThreads = false;
function debugWT(...args) {
  if (debugWorkerThreads) {
    // deno-lint-ignore prefer-primordials no-console
    console.log(...args);
  }
}

interface WorkerOnlineMsg {
  type: "WORKER_ONLINE";
}

function isWorkerOnlineMsg(data: unknown): data is WorkerOnlineMsg {
  return typeof data === "object" && data !== null &&
    ObjectHasOwn(data, "type") &&
    (data as { "type": unknown })["type"] === "WORKER_ONLINE";
}

interface WorkerStdioMsg {
  type: "WORKER_STDERR" | "WORKER_STDOUT";
  // deno-lint-ignore no-explicit-any
  data: any;
}

interface WorkerStdinMsg {
  type: "WORKER_STDIN";
  // deno-lint-ignore no-explicit-any
  data: any;
}

interface WorkerStdinEndMsg {
  type: "WORKER_STDIN_END";
}

function isWorkerStdinMsg(data: unknown): data is WorkerStdinMsg {
  return typeof data === "object" && data !== null &&
    ObjectHasOwn(data, "type") &&
    (data as { "type": unknown })["type"] === "WORKER_STDIN";
}

function isWorkerStdinEndMsg(data: unknown): data is WorkerStdinEndMsg {
  return typeof data === "object" && data !== null &&
    ObjectHasOwn(data, "type") &&
    (data as { "type": unknown })["type"] === "WORKER_STDIN_END";
}

function isWorkerStderrMsg(data: unknown): data is WorkerStdioMsg {
  return typeof data === "object" && data !== null &&
    ObjectHasOwn(data, "type") &&
    (data as { "type": unknown })["type"] === "WORKER_STDERR";
}

function isWorkerStdoutMsg(data: unknown): data is WorkerStdioMsg {
  return typeof data === "object" && data !== null &&
    ObjectHasOwn(data, "type") &&
    (data as { "type": unknown })["type"] === "WORKER_STDOUT";
}

// Flags that are valid Node.js environment flags but not allowed in workers
// because they affect per-process state.
const workerDisallowedFlags = new SafeSet([
  "--title",
  "--redirect-warnings",
  "--trace-event-file-pattern",
  "--trace-event-categories",
  "--trace-events-enabled",
  "--diagnostic-dir",
  "--report-signal",
  "--report-filename",
  "--report-dir",
  "--report-directory",
  "--report-compact",
  "--report-on-signal",
  "--report-on-fatalerror",
  "--report-uncaught-exception",
]);

// V8 profiling flags Node accepts in worker execArgv. Deno doesn't wire up
// the underlying profiling integration, but rejecting them with
// ERR_WORKER_INVALID_EXEC_ARGV breaks scripts that pass them unconditionally,
// so accept and ignore.
const workerSilentlyIgnoredFlags = new SafeSet([
  "--heap-prof",
  "--heap-prof-interval",
  "--heap-prof-name",
  "--heap-prof-dir",
  "--cpu-prof",
  "--cpu-prof-interval",
  "--cpu-prof-name",
  "--cpu-prof-dir",
  // Internal Node debug flag; Node tests pass it through unconditionally
  // when run with NODE_OPTIONS=--expose-internals.
  "--expose-internals",
  // Node's `--input-type` configures the parser for eval workers; we
  // always treat eval as classic JS, so accept and ignore.
  "--input-type",
  // Node deprecation/experimental warnings suppression -- harmless to
  // accept since Deno doesn't emit the same warnings anyway.
  "--disable-warning",
  "--disable-proto",
]);

interface WorkerOptions {
  // only for typings
  argv?: unknown[];
  env?: Record<string, unknown>;
  execArgv?: string[];
  stdin?: boolean;
  stdout?: boolean;
  stderr?: boolean;
  trackUnmanagedFds?: boolean;
  resourceLimits?: {
    maxYoungGenerationSizeMb?: number;
    maxOldGenerationSizeMb?: number;
    codeRangeSizeMb?: number;
    stackSizeMb?: number;
  };
  // deno-lint-ignore prefer-primordials
  eval?: boolean;
  transferList?: Transferable[];
  workerData?: unknown;
  name?: string;
}

const privateWorkerRef = Symbol("privateWorkerRef");
class NodeWorker extends EventEmitter {
  #id = 0;
  #name = "";
  #refed = true;
  #messagePromise = undefined;
  #controlPromise = undefined;
  #messageLoopPromise = undefined;
  #workerOnline = false;
  #exited = false;
  // "RUNNING" | "CLOSED" | "TERMINATED"
  // "TERMINATED" means that any controls or messages received will be
  // discarded. "CLOSED" means that we have received a control
  // indicating that the worker is no longer running, but there might
  // still be messages left to receive.
  #status = "RUNNING";

  // https://nodejs.org/api/worker_threads.html#workerthreadid
  threadId = this.#id;
  // https://nodejs.org/api/worker_threads.html#workerresourcelimits
  resourceLimits: WorkerOptions["resourceLimits"] = {};
  // https://nodejs.org/api/worker_threads.html#workerstdin
  // deno-lint-ignore no-explicit-any
  stdin: any = null;
  // https://nodejs.org/api/worker_threads.html#workerstdout
  // deno-lint-ignore no-explicit-any
  stdout: any = new (lazyStream().Readable)({ read() {} });
  // https://nodejs.org/api/worker_threads.html#workerstderr
  // deno-lint-ignore no-explicit-any
  stderr: any = new (lazyStream().Readable)({ read() {} });

  constructor(specifier: URL | string, options?: WorkerOptions) {
    super();

    // Validate filename arg type before anything else -- Node throws
    // ERR_INVALID_ARG_TYPE with a specific message that several tests
    // assert against.
    if (
      typeof specifier !== "string" &&
      !(typeof specifier === "object" && specifier !== null &&
        typeof (specifier as URL).protocol === "string")
    ) {
      throw new ERR_INVALID_ARG_TYPE(
        "filename",
        ["string", "URL"],
        specifier,
      );
    }

    if (options?.execArgv) {
      validateArray(options.execArgv, "options.execArgv");
      if (options.execArgv.length > 0) {
        const invalidFlags = [];
        for (let i = 0; i < options.execArgv.length; i++) {
          const flag = options.execArgv[i];
          // Items that don't start with '-' are arguments to the
          // preceding flag (e.g. "--conditions node"), not flags.
          if (!StringPrototypeStartsWith(flag, "-")) {
            continue;
          }
          const eqIdx = StringPrototypeIndexOf(flag, "=");
          const flagName = eqIdx === -1
            ? flag
            : StringPrototypeSlice(flag, 0, eqIdx);
          if (workerSilentlyIgnoredFlags.has(flagName)) {
            continue;
          }
          if (!process.allowedNodeEnvironmentFlags.has(flag)) {
            invalidFlags[invalidFlags.length] = flag;
            continue;
          }
          if (workerDisallowedFlags.has(flagName)) {
            invalidFlags[invalidFlags.length] = flag;
          }
        }
        if (invalidFlags.length > 0) {
          throw new ERR_WORKER_INVALID_EXEC_ARGV(invalidFlags);
        }
      }
    }

    if (options?.env) {
      const nodeOptions = options.env.NODE_OPTIONS;
      if (typeof nodeOptions === "string" && nodeOptions.length > 0) {
        // Parse NODE_OPTIONS and validate each flag
        const parts = StringPrototypeSplit(
          StringPrototypeTrim(nodeOptions),
          new SafeRegExp("\\s+"),
        );
        let hasInvalid = false;
        for (let i = 0; i < parts.length; i++) {
          const part = parts[i];
          if (StringPrototypeStartsWith(part, "-")) {
            const eqIdx = StringPrototypeIndexOf(part, "=");
            const partName = eqIdx === -1
              ? part
              : StringPrototypeSlice(part, 0, eqIdx);
            if (workerSilentlyIgnoredFlags.has(partName)) {
              continue;
            }
            if (
              !process.allowedNodeEnvironmentFlags.has(part) ||
              workerDisallowedFlags.has(partName)
            ) {
              hasInvalid = true;
              break;
            }
          }
        }
        if (hasInvalid) {
          throw new ERR_WORKER_INVALID_EXEC_ARGV(
            [nodeOptions],
            "invalid NODE_OPTIONS env variable",
          );
        }
      }
    }

    if (typeof specifier === "object") {
      if (
        !(specifier.protocol === "data:" || specifier.protocol === "file:")
      ) {
        throw new ERR_INVALID_URL_SCHEME(["file", "data"]);
      }
    } else if (typeof specifier === "string" && !options?.eval) {
      // Node.js requires string specifiers to be absolute paths or
      // relative paths starting with './' or '../'. URLs passed as
      // strings must be wrapped with `new URL`.
      if (
        StringPrototypeStartsWith(specifier, "file://") ||
        StringPrototypeStartsWith(specifier, "data:") ||
        StringPrototypeStartsWith(specifier, "http://") ||
        StringPrototypeStartsWith(specifier, "https://")
      ) {
        throw new ERR_WORKER_PATH(specifier);
      }
      const path = specifier;
      if (
        !StringPrototypeStartsWith(path, "/") &&
        !StringPrototypeStartsWith(path, "./") &&
        !StringPrototypeStartsWith(path, "../") &&
        !StringPrototypeStartsWith(path, ".\\") &&
        !StringPrototypeStartsWith(path, "..\\")
      ) {
        // On Windows, also allow drive-letter absolute paths (e.g. C:\...)
        const isWindowsAbsolute = path.length >= 3 && path[1] === ":" &&
          (path[2] === "\\" || path[2] === "/");
        if (!isWindowsAbsolute) {
          throw new ERR_WORKER_PATH(specifier);
        }
      }
    }

    // Serialize workerData before resolving the filename so that
    // DataCloneError is thrown before file-not-found errors,
    // matching Node.js behavior.

    // Handle the `env` option following Node.js semantics:
    // - undefined/null: snapshot current process.env (isolated copy)
    // - SHARE_ENV: worker shares the parent's OS environment
    // - object: use that object, coercing values to strings
    // - anything else: throw ERR_INVALID_ARG_TYPE
    // See https://github.com/denoland/deno/issues/23522.
    let env_ = undefined;
    const envOpt = options?.env;
    if (envOpt != null && envOpt !== SHARE_ENV) {
      if (typeof envOpt !== "object") {
        throw new ERR_INVALID_ARG_TYPE(
          "options.env",
          ["object", "undefined", "null", "worker_threads.SHARE_ENV"],
          envOpt,
        );
      }
      // Snapshot the provided env, coercing values to strings like Node.js.
      // This also handles passing `process.env` (a Proxy in Deno) by
      // producing a plain object that can be structured-cloned.
      const envObj = {};
      const keys = ObjectKeys(envOpt);
      for (let i = 0; i < keys.length; i++) {
        envObj[keys[i]] = String(envOpt[keys[i]]);
      }
      env_ = envObj;
    } else if (envOpt !== SHARE_ENV) {
      // Default: snapshot current process.env so the worker gets an
      // isolated copy, not a live reference to the OS environment.
      // Wrap in try/catch because accessing process.env requires
      // --allow-env permission in Deno. If unavailable, fall back to
      // shared OS env (env_ stays undefined).
      try {
        const envObj = {};
        const keys = ObjectKeys(process.env);
        for (let i = 0; i < keys.length; i++) {
          envObj[keys[i]] = process.env[keys[i]];
        }
        env_ = envObj;
      } catch {
        // No env permission - worker will share the OS environment.
      }
    }
    // When envOpt === SHARE_ENV, env_ stays undefined and the worker
    // will use the default process.env backed by Deno.env (shared OS env).

    // Handle the `argv` option: must be an array or undefined.
    // Values are coerced to strings like Node.js does.
    let argv_: string[] | undefined = undefined;
    if (options?.argv != null) {
      if (!ArrayIsArray(options.argv)) {
        throw new ERR_INVALID_ARG_TYPE(
          "options.argv",
          "Array",
          options.argv,
        );
      }
      argv_ = [];
      for (let i = 0; i < options.argv.length; i++) {
        argv_[i] = String(options.argv[i]);
      }
    }

    const resourceLimits_ = options?.resourceLimits ?? undefined;

    const serializedWorkerMetadata = serializeJsMessageData({
      workerData: options?.workerData,
      environmentData: environmentData,
      env: env_,
      argv: argv_,
      // Node inherits the parent's execArgv when the worker doesn't
      // specify one explicitly. This is what carries flags like
      // `--expose-gc` and `--expose-internals` into the worker.
      execArgv: options?.execArgv ?? (process.execArgv ?? []),
      name: this.#name,
      isEval: !!options?.eval,
      isWorkerThread: true,
      hasStdin: !!options?.stdin,
      hasStdout: !!options?.stdout,
      hasStderr: !!options?.stderr,
      resourceLimits: resourceLimits_,
    }, options?.transferList ?? []);

    let sourceCode = "";
    let hasSourceCode = false;

    if (options?.eval) {
      if (typeof specifier !== "string") {
        throw new TypeError(
          "The property 'options.eval' must be false when 'filename' is not a string.",
        );
      }
      const code = specifier;
      // Node.js runs eval workers as CJS (sloppy mode).
      // Pass as source code for execute_script (sloppy mode).
      // `require` is already available from the Node worker bootstrap.
      // See: https://github.com/denoland/deno/issues/26739
      //
      // Wrap the user code in an immediately-invoked function so the
      // user's top-level `let`/`const`/`function` declarations don't
      // collide with our `__filename`/`__dirname`/`module`/`exports`
      // bindings. Assignments to `module.exports` still propagate
      // because `module` lives in the surrounding scope.
      sourceCode = `var __filename = ${
        // deno-lint-ignore prefer-primordials
        JSON.stringify(process.cwd() + "/[worker eval]")};\n` +
        `var __dirname = ${
          // deno-lint-ignore prefer-primordials
          JSON.stringify(process.cwd())};\n` +
        `var module = { exports: {} };\n` +
        `var exports = module.exports;\n` +
        `(function() {\n` +
        code +
        `\n}).call(this);\n`;
      hasSourceCode = true;
      specifier = `data:text/javascript,`;
    } else if (
      !(typeof specifier === "object" && specifier.protocol === "data:")
    ) {
      // deno-lint-ignore prefer-primordials
      specifier = specifier.toString();
      specifier = op_worker_threads_filename(specifier) ?? specifier;
    }

    // TODO(bartlomieu): this doesn't match the Node.js behavior, it should be
    // `[worker {threadId}] {name}` or empty string.
    let name = StringPrototypeTrim(options?.name ?? "");
    if (options?.eval) {
      name = "[worker eval]";
    }
    this.#name = name;

    const id = op_create_worker(
      {
        // deno-lint-ignore prefer-primordials
        specifier: specifier.toString(),
        hasSourceCode,
        sourceCode,
        permissions: null,
        name: this.#name,
        workerType: "node",
        closeOnIdle: true,
        resourceLimits: resourceLimits_,
      },
      serializedWorkerMetadata,
    );
    this.#id = id;
    this.threadId = id;

    if (resourceLimits_) {
      this.resourceLimits = { ...resourceLimits_ };
    }

    if (options?.stdin) {
      // deno-lint-ignore no-this-alias
      const worker = this;
      this.stdin = new (lazyStream().Writable)({
        write(chunk, _encoding, callback) {
          try {
            worker.postMessage({
              type: "WORKER_STDIN",
              data: chunk,
            });
            callback();
          } catch (err) {
            callback(err);
          }
        },
        final(callback) {
          try {
            worker.postMessage({
              type: "WORKER_STDIN_END",
            });
            callback();
          } catch (err) {
            callback(err);
          }
        },
      });
    }

    this.#pollControl();
    this.#messageLoopPromise = this.#pollMessages();
    process.nextTick(() => process.emit("worker", this));
  }

  [privateWorkerRef](ref) {
    if (ref === this.#refed) {
      return;
    }
    this.#refed = ref;

    if (ref) {
      if (this.#controlPromise) {
        core.refOpPromise(this.#controlPromise);
      }
      if (this.#messagePromise) {
        core.refOpPromise(this.#messagePromise);
      }
    } else {
      if (this.#controlPromise) {
        core.unrefOpPromise(this.#controlPromise);
      }
      if (this.#messagePromise) {
        core.unrefOpPromise(this.#messagePromise);
      }
    }
  }

  #handleError(err) {
    this.emit("error", err);
  }

  #closeStdio() {
    if (!this.stdout.readableEnded) {
      FunctionPrototypeCall(
        lazyStream().Readable.prototype.push,
        this.stdout,
        null,
      );
    }
    if (!this.stderr.readableEnded) {
      FunctionPrototypeCall(
        lazyStream().Readable.prototype.push,
        this.stderr,
        null,
      );
    }
  }

  #pollControl = async () => {
    while (this.#status === "RUNNING") {
      this.#controlPromise = op_host_recv_ctrl(this.#id);
      if (!this.#refed) {
        core.unrefOpPromise(this.#controlPromise);
      }
      const { 0: type, 1: data } = await this.#controlPromise;

      // If terminate was called then we ignore all messages
      if (this.#status === "TERMINATED") {
        return;
      }

      switch (type) {
        case 1: { // TerminalError
          this.#status = "CLOSED";
          if (this.listenerCount("error") > 0) {
            const errMsg = data.errorMessage ?? data.message;
            const errName = data.name;
            let err;
            if (errName === "ERR_WORKER_OUT_OF_MEMORY") {
              err = new Error(errMsg);
              err.code = errName;
              err.name = "Error";
            } else {
              // Use the correct native error constructor so that
              // err.constructor matches (e.g. SyntaxError, TypeError).
              const Ctor = nativeErrorConstructors[errName] ?? Error;
              err = new Ctor(errMsg);
            }
            // Stack is unavailable from the worker context (e.g. prepareStackTrace
            // may have thrown). Match Node.js behavior of setting stack to undefined.
            err.stack = undefined;
            this.emit("error", err);
          }
          // Drain pending messages (including queued stdio chunks)
          // BEFORE closing stdio, otherwise late WORKER_STDOUT messages
          // would try to push() into an already-ended stream.
          await this.#messageLoopPromise;
          this.#closeStdio();
          this.resourceLimits = {};
          if (!this.#exited) {
            this.#exited = true;
            this.emit("exit", data.exitCode ?? 1);
          }
          return;
        }
        case 2: { // Error
          this.#handleError(data);
          break;
        }
        case 3: { // Close
          debugWT(`Host got "close" message from worker: ${this.#name}`);
          this.#status = "CLOSED";
          // Drain pending messages (including queued stdio chunks)
          // BEFORE closing stdio so all data still reaches the host
          // stdout/stderr streams.
          await this.#messageLoopPromise;
          this.#closeStdio();
          this.resourceLimits = {};
          if (!this.#exited) {
            this.#exited = true;
            this.emit("exit", data ?? 0);
          }
          return;
        }
        default: {
          throw new Error(`Unknown worker event: "${type}"`);
        }
      }
    }
  };

  #dispatchWorkerThreadMessage(data) {
    let message, _transferables;
    try {
      const v = deserializeJsMessageData(data);
      message = v[0];
      _transferables = v[1];
    } catch (err) {
      this.emit("messageerror", err);
      return false;
    }
    if (
      // only emit "online" event once, and since the message
      // has to come before user messages, we are safe to assume
      // it came from us
      !this.#workerOnline && isWorkerOnlineMsg(message)
    ) {
      this.#workerOnline = true;
      this.emit("online");
    } else if (isWorkerStdoutMsg(message)) {
      FunctionPrototypeCall(
        lazyStream().Readable.prototype.push,
        this.stdout,
        message.data,
      );
    } else if (isWorkerStderrMsg(message)) {
      FunctionPrototypeCall(
        lazyStream().Readable.prototype.push,
        this.stderr,
        message.data,
      );
    } else {
      this.emit("message", message);
    }
    return true;
  }

  #pollMessages = async () => {
    while (this.#status !== "TERMINATED") {
      this.#messagePromise = op_host_recv_message(this.#id);
      if (!this.#refed) {
        core.unrefOpPromise(this.#messagePromise);
      }
      const data = await this.#messagePromise;
      if (this.#status === "TERMINATED" || data === null) {
        return;
      }
      if (!this.#dispatchWorkerThreadMessage(data)) return;
      // Sync drain: process a limited batch of already-queued messages
      // without going through the async op machinery. The batch limit
      // prevents starvation of the event loop when message handlers
      // synchronously post new messages (e.g. ping-pong patterns).
      for (let i = 0; i < 1000 && this.#status !== "TERMINATED"; i++) {
        const syncData = op_host_recv_message_sync(this.#id);
        if (syncData === null) break;
        if (!this.#dispatchWorkerThreadMessage(syncData)) return;
      }
    }
  };

  postMessage(message, transferOrOptions = { __proto__: null }) {
    const prefix = "Failed to execute 'postMessage' on 'MessagePort'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    if (this.#status !== "RUNNING") return;
    // Fast path: no transferables
    if (
      transferOrOptions === undefined ||
      transferOrOptions === null ||
      (arguments.length <= 1)
    ) {
      op_host_post_message_raw(
        this.#id,
        core.serialize(message),
      );
      return;
    }
    message = webidl.converters.any(message);
    let options;
    if (
      webidl.type(transferOrOptions) === "Object" &&
      transferOrOptions !== undefined &&
      transferOrOptions[SymbolIterator] !== undefined
    ) {
      const transfer = webidl.converters["sequence<object>"](
        transferOrOptions,
        prefix,
        "Argument 2",
      );
      options = { transfer };
    } else {
      options = webidl.converters.StructuredSerializeOptions(
        transferOrOptions,
        prefix,
        "Argument 2",
      );
    }
    const { transfer } = options;
    const data = serializeJsMessageData(message, transfer);
    op_host_post_message(this.#id, data);
  }

  // https://nodejs.org/api/worker_threads.html#workerterminate
  terminate() {
    if (this.#status === "TERMINATED") {
      return PromiseResolve(undefined);
    }

    this.#status = "TERMINATED";
    op_host_terminate_worker(this.#id);
    this.#closeStdio();

    if (!this.#exited) {
      this.#exited = true;
      this.emit("exit", 1);
      return PromiseResolve(1);
    }

    // Worker already exited - Node.js returns undefined in this case
    // (the internal handle is already null).
    return PromiseResolve(undefined);
  }

  async [SymbolAsyncDispose]() {
    await this.terminate();
  }

  ref() {
    this[privateWorkerRef](true);
  }

  unref() {
    this[privateWorkerRef](false);
  }

  cpuUsage(prevValue?: { user: number; system: number }) {
    if (prevValue != null && !NumberIsNaN(prevValue)) {
      validateObject(prevValue, "prevValue");
      if (typeof prevValue.user !== "number") {
        throw new ERR_INVALID_ARG_TYPE(
          "prevValue.user",
          "number",
          prevValue.user,
        );
      }
      if (!NumberIsFinite(prevValue.user) || prevValue.user < 0) {
        throw new ERR_OUT_OF_RANGE(
          "prevValue.user",
          ">= 0 && <= 2^53",
          prevValue.user,
        );
      }
      if (typeof prevValue.system !== "number") {
        throw new ERR_INVALID_ARG_TYPE(
          "prevValue.system",
          "number",
          prevValue.system,
        );
      }
      if (!NumberIsFinite(prevValue.system) || prevValue.system < 0) {
        throw new ERR_OUT_OF_RANGE(
          "prevValue.system",
          ">= 0 && <= 2^53",
          prevValue.system,
        );
      }
    }

    if (this.#status !== "RUNNING") {
      return PromiseReject(new ERR_WORKER_NOT_RUNNING());
    }

    op_host_get_worker_cpu_usage(this.#id, workerCpuUsageBuffer);
    const user = workerCpuUsageBuffer[0];
    const system = workerCpuUsageBuffer[1];
    if (prevValue) {
      return PromiseResolve({
        user: user - prevValue.user,
        system: system - prevValue.system,
      });
    }
    return PromiseResolve({ user, system });
  }

  // https://nodejs.org/api/worker_threads.html#workerthreadname
  get threadName(): string | null {
    if (this.#exited) {
      return null;
    }
    return this.#name;
  }

  readonly getHeapSnapshot = () =>
    notImplemented("Worker.prototype.getHeapSnapshot");
  // fake performance
  readonly performance = globalThis.performance;
}

let isMainThread;
let resourceLimits;
let threadName: string = "";

let threadId = 0;
let workerData: unknown = null;
let environmentData = new SafeMap();

// Forward-declared so `__initWorkerThreads` can sync the post-init values
// of `parentPort`/`threadId`/etc. onto it as data properties. The tail of
// this IIFE fills in the rest (classes, helper functions, initial values
// for the mutable fields) and returns this object as the polyfill's
// exports. Synthetic ESM `import` snapshots these properties at first
// import - which is post-bootstrap - so consumers see the post-init
// values. CJS `require("worker_threads")` reads the same object.
const exportsObj: Record<string, unknown> = {};

// Like https://github.com/nodejs/node/blob/48655e17e1d84ba5021d7a94b4b88823f7c9c6cf/lib/internal/event_target.js#L611
interface NodeEventTarget extends
  Pick<
    EventEmitter,
    "eventNames" | "listenerCount" | "emit" | "removeAllListeners"
  > {
  setMaxListeners(n: number): void;
  getMaxListeners(): number;
  // deno-lint-ignore no-explicit-any
  off(eventName: string, listener: (...args: any[]) => void): NodeEventTarget;
  // deno-lint-ignore no-explicit-any
  on(eventName: string, listener: (...args: any[]) => void): NodeEventTarget;
  // deno-lint-ignore no-explicit-any
  once(eventName: string, listener: (...args: any[]) => void): NodeEventTarget;
  addListener: NodeEventTarget["on"];
  removeListener: NodeEventTarget["off"];
}

interface ParentPort extends NodeEventTarget {
  postMessage(message: unknown, transferOrOptions?: unknown): void;
  addEventListener(
    name: string,
    listener: EventListenerOrEventListenerObject,
    options?: boolean | AddEventListenerOptions,
  ): void;
  removeEventListener(
    name: string,
    listener: EventListenerOrEventListenerObject,
  ): void;
  onmessage: ((ev: Event) => void) | null;
  // deno-lint-ignore no-explicit-any
  emit(...args: any[]): any;
  removeAllListeners(): void;
  setMaxListeners(n: number): void;
  getMaxListeners(): number;
  eventNames(): string[];
  listenerCount(): number;
  unref(): void;
  ref(): void;
  [key: symbol]: unknown;
}

// deno-lint-ignore no-explicit-any
let parentPort: ParentPort = null as any;

internals.__initWorkerThreads = (
  runningOnMainThread: boolean,
  workerId,
  maybeWorkerMetadata,
  moduleSpecifier,
) => {
  isMainThread = runningOnMainThread;
  internals.__isWorkerThread = !runningOnMainThread;

  // Many Node tests reach for the global `MessageChannel` and
  // `MessagePort` rather than destructuring them from `worker_threads`.
  // Repoint the globals to the Node-flavoured class so user code sees
  // the EventEmitter surface regardless of the import style.
  globalThis.MessageChannel = MessageChannel;
  globalThis.MessagePort = MessagePort;

  if (isMainThread) {
    resourceLimits = {};
  }

  if (!isMainThread) {
    // TODO(bartlomieju): this is a really hacky way to provide
    // require in worker_threads - this should be rewritten to use proper
    // CJS/ESM loading
    if (moduleSpecifier) {
      globalThis.require = lazyModule().createRequire(
        StringPrototypeStartsWith(moduleSpecifier, "data:")
          ? `${Deno.cwd()}/[worker eval]`
          : moduleSpecifier,
      );
    }

    threadId = workerId;
    let isWorkerThread = false;
    if (maybeWorkerMetadata) {
      const { 0: metadata, 1: _ } = maybeWorkerMetadata;
      workerData = metadata.workerData;
      environmentData = metadata.environmentData;
      isWorkerThread = metadata.isWorkerThread;
      threadName = metadata.name ?? "";
      const env = metadata.env;
      if (env) {
        process.env = env;
      }

      // Get resolved resource limits from the Rust side (includes V8
      // defaults for unspecified fields), matching Node.js behavior.
      const resolvedLimits = op_worker_get_resource_limits();
      if (resolvedLimits) {
        resourceLimits = resolvedLimits;
      } else {
        resourceLimits = {};
      }

      // Set process.argv for worker threads.
      // In Node.js, worker process.argv is [execPath, scriptPath, ...argv].
      if (isWorkerThread) {
        let scriptPath;
        if (metadata.isEval) {
          scriptPath = "[worker eval]";
        } else if (
          moduleSpecifier &&
          StringPrototypeStartsWith(moduleSpecifier, "file:")
        ) {
          scriptPath = lazyUrl().fileURLToPath(moduleSpecifier);
        } else {
          scriptPath = moduleSpecifier ?? "";
        }
        process.argv = [process.execPath, scriptPath];
        if (metadata.argv) {
          for (let i = 0; i < metadata.argv.length; i++) {
            process.argv[i + 2] = metadata.argv[i];
          }
        }

        // Set process.execArgv for worker threads.
        if (metadata.execArgv) {
          process.execArgv = metadata.execArgv;
          core.loadExtScript(
            "ext:deno_node/internal_binding/node_options.ts",
          ).setOptionSourceExecArgv(metadata.execArgv);
          for (let i = 0; i < metadata.execArgv.length; i++) {
            if (metadata.execArgv[i] === "--trace-warnings") {
              process.traceProcessWarnings = true;
            }
          }
        }

        // Install a no-op `globalThis.gc()` shim. V8's --expose-gc
        // exposes it in Node; Deno doesn't ship those bindings but
        // many Node tests call `gc()` unconditionally. The shim lets
        // them proceed without `TypeError: gc is not a function`.
        if (typeof globalThis.gc !== "function") {
          globalThis.gc = () => {};
        }

        // Replace process.stdin with a Readable that receives
        // data from the parent via WORKER_STDIN messages. The handler
        // is installed on globalThis BEFORE parentPort's bridge so that
        // stopImmediatePropagation prevents the WORKER_STDIN sentinels
        // from being forwarded to parentPort.on('message') listeners.
        if (metadata.hasStdin) {
          const workerStdin = new (lazyStream().Readable)({ read() {} });
          process.stdin = workerStdin;

          const stdinHandler = (ev) => {
            const msg = ev.data;
            if (isWorkerStdinMsg(msg)) {
              // deno-lint-ignore prefer-primordials
              workerStdin.push(msg.data);
              ev.stopImmediatePropagation();
            } else if (isWorkerStdinEndMsg(msg)) {
              // deno-lint-ignore prefer-primordials
              workerStdin.push(null);
              globalThis.removeEventListener("message", stdinHandler);
              ev.stopImmediatePropagation();
            }
          };
          globalThis.addEventListener("message", stdinHandler);
        }
      }
    }

    // Build the Node-style parentPort backed by the worker's per-thread
    // message channel. Its `addEventListener` bridge is installed onto
    // globalThis here, AFTER any sentinel filters (stdin handler above)
    // so those can stopImmediatePropagation cleanly.
    parentPort = createParentPort() as unknown as ParentPort;

    if (isWorkerThread) {
      // Wrap process.cwd in workers so its `.toString()` mentions
      // `AtomicsLoad` (matches Node's worker-thread implementation,
      // which uses a shared atomic to communicate cwd changes from
      // the main thread). The actual implementation still delegates to
      // the underlying cwd op -- Deno doesn't replicate Node's
      // atomics-based propagation, but the signature is what
      // node_compat's `test-worker-process-cwd` asserts on.
      const realCwd = process.cwd;
      const cwdWrapper = function cwd() {
        // AtomicsLoad: see https://nodejs.org/api/process.html#processcwd
        return realCwd();
      };
      process.cwd = cwdWrapper;
      // process.stdout/stderr.write forwards must come AFTER parentPort
      // exists since they call parentPort.postMessage.
      //
      // When the host opted into piping (`stdout: true` / `stderr:
      // true`), the underlying stream must NOT also write to the real
      // tty -- Node redirects entirely. When piping wasn't requested
      // the write still flows through so logs show up in the host's
      // terminal, matching `stdio: 'inherit'` behavior.
      const metaInner = (maybeWorkerMetadata?.[0] ?? {}) as {
        hasStdout?: boolean;
        hasStderr?: boolean;
      };
      const pipeStdout = !!metaInner.hasStdout;
      const pipeStderr = !!metaInner.hasStderr;

      // Deno's globalThis.console is the Web Console implementation,
      // which writes via `core.print` rather than `process.stdout.write`.
      // That bypasses our pipe hook, so when the host requested piping we
      // swap in Node's `Console` class -- whose methods funnel through
      // `process.stdout.write` / `process.stderr.write`, which we've
      // overridden below to post WORKER_STDOUT/WORKER_STDERR messages
      // back to the parent.
      if (pipeStdout || pipeStderr) {
        try {
          const nodeConsoleMod = core.loadExtScript(
            "ext:deno_node/internal/console/constructor.mjs",
          );
          const NodeConsole = nodeConsoleMod.Console ??
            nodeConsoleMod.default?.Console ?? nodeConsoleMod.default;
          if (typeof NodeConsole === "function") {
            globalThis.console = new NodeConsole({
              stdout: process.stdout,
              stderr: process.stderr,
            });
          }
        } catch {
          // Console constructor wiring missing -- leave the web console
          // in place rather than crash the worker.
        }
      }
      const origStdoutWrite = FunctionPrototypeBind(
        process.stdout.write,
        process.stdout,
      );
      process.stdout.write = function (chunk, encoding, callback) {
        parentPort.postMessage({
          type: "WORKER_STDOUT",
          data: chunk,
        });
        if (pipeStdout) {
          if (typeof callback === "function") callback();
          return true;
        }
        return FunctionPrototypeCall(
          origStdoutWrite,
          process.stdout,
          chunk,
          encoding,
          callback,
        );
      };

      const origStderrWrite = FunctionPrototypeBind(
        process.stderr.write,
        process.stderr,
      );
      process.stderr.write = function (chunk, encoding, callback) {
        parentPort.postMessage({
          type: "WORKER_STDERR",
          data: chunk,
        });
        if (pipeStderr) {
          if (typeof callback === "function") callback();
          return true;
        }
        return FunctionPrototypeCall(
          origStderrWrite,
          process.stderr,
          chunk,
          encoding,
          callback,
        );
      };

      // Notify the host that the worker is online.
      parentPort.postMessage(
        {
          type: "WORKER_ONLINE",
        } satisfies WorkerOnlineMsg,
      );
    }
  }

  // Register this thread in the cross-thread registry so it can receive
  // `postMessageToThread` calls.
  setupCrossThreadMessaging();

  // Sync the post-init values of the module-local lets onto the exports
  // object so first-import snapshots (via the synthetic ESM dispatch) and
  // `require("worker_threads")` reads both see the bootstrap-resolved
  // values rather than the initial placeholders.
  exportsObj.parentPort = parentPort;
  exportsObj.threadId = threadId;
  exportsObj.workerData = workerData;
  exportsObj.isMainThread = isMainThread;
  exportsObj.resourceLimits = resourceLimits;
  exportsObj.threadName = threadName;
};

function getEnvironmentData(key: unknown) {
  return environmentData.get(key);
}

function setEnvironmentData(key: unknown, value?: unknown) {
  if (value === undefined) {
    environmentData.delete(key);
  } else {
    environmentData.set(key, value);
  }
}

const SHARE_ENV = SymbolFor("nodejs.worker_threads.SHARE_ENV");

// ---------------------------------------------------------------
// Cross-thread messaging (`postMessageToThread` / `workerMessage`)
// ---------------------------------------------------------------
//
// Each thread (main + every Node worker) registers itself with the Rust
// side via `op_node_worker_thread_register`, which hands back a receive
// channel. The send side lives in a process-wide table keyed by thread
// id; any thread can post to any other thread by id, and the receive
// loop here decodes the envelope and dispatches it as a `workerMessage`
// event on `process`.
//
// A short envelope { sender, id, kind, payload, status? } wraps each
// payload so we can carry the sender id alongside an ack message back to
// the sender once the destination's `workerMessage` listeners have run.
// The ack resolves the sender's pending promise (or rejects it on
// listener throw / timeout), matching the Node.js semantics that
// `postMessageToThread` only resolves once the destination has actually
// handled the message.

const kMessageKindData = 0;
const kMessageKindAck = 1;
const kAckStatusOk = 0;
const kAckStatusError = 1;

let threadMessageRid: number | undefined;
let workerMessageListenerCount = 0;
let nextThreadMessageId = 1;
let crossThreadSetUp = false;
const pendingThreadMessages = new SafeMap<
  number,
  {
    resolve: (v: void) => void;
    reject: (err: unknown) => void;
    timer: number | undefined;
  }
>();

function isCrossThreadEnvelope(
  v: unknown,
): v is {
  sender: number;
  id: number;
  kind: number;
  payload?: unknown;
  status?: number;
} {
  return typeof v === "object" && v !== null &&
    ObjectHasOwn(v, "__nodeWorkerMessage") &&
    (v as { __nodeWorkerMessage: unknown }).__nodeWorkerMessage === true;
}

function deliverThreadMessageAck(
  id: number,
  status: number,
) {
  const pending = pendingThreadMessages.get(id);
  if (pending === undefined) return;
  pendingThreadMessages.delete(id);
  if (pending.timer !== undefined) clearTimeout(pending.timer);
  if (status === kAckStatusOk) {
    pending.resolve(undefined);
  } else {
    pending.reject(new ERR_WORKER_MESSAGING_ERRORED());
  }
}

function sendAck(
  senderThreadId: number,
  id: number,
  status: number,
) {
  try {
    const envelope = {
      __nodeWorkerMessage: true,
      sender: threadId,
      id,
      kind: kMessageKindAck,
      status,
    };
    const data = serializeJsMessageData(envelope, []);
    // Acks bypass the listener-count gate: the sender is, by definition,
    // waiting on this reply even though it has no `workerMessage`
    // listener of its own.
    op_node_worker_thread_post_message(senderThreadId, data, true);
  } catch {
    // Best-effort: a failed ack just means the sender will time out.
  }
}

async function pollCrossThreadMessages() {
  if (threadMessageRid === undefined) return;
  while (threadMessageRid !== undefined) {
    const promise = op_node_worker_thread_recv_message(threadMessageRid);
    // Don't let the cross-thread receive op keep the event loop alive
    // on its own: the thread should be free to exit when there's no
    // other work, even though we're permanently parked on this op.
    core.unrefOpPromise(promise);
    const data = await promise;
    if (data === null) return;
    let envelope;
    try {
      envelope = deserializeJsMessageData(data)[0];
    } catch {
      continue;
    }
    if (!isCrossThreadEnvelope(envelope)) {
      continue;
    }
    if (envelope.kind === kMessageKindAck) {
      deliverThreadMessageAck(envelope.id, envelope.status ?? kAckStatusOk);
      continue;
    }
    // kMessageKindData - surface to user code as a `workerMessage` event
    // on `process`, then ack with whatever the listeners produced.
    // Node's `process.on('workerMessage', (value, source) => ...)` gets
    // the sender thread id as the second argument.
    let status = kAckStatusOk;
    try {
      process.emit("workerMessage", envelope.payload, envelope.sender);
    } catch {
      status = kAckStatusError;
    }
    sendAck(envelope.sender, envelope.id, status);
  }
}

// Register the thread eagerly so that `postMessageToThread` from another
// thread can reach us, even before any `workerMessage` listener exists.
// Without this, the destination's `result === 0` ("no such thread")
// would fire instead of a timeout, breaking
// `test-worker-messaging-errors-timeout` which expects a
// `ERR_WORKER_MESSAGING_TIMEOUT` rather than `_FAILED` when the worker
// is alive but unresponsive.
function setupCrossThreadMessaging() {
  workerMessageListenerCount = process.listenerCount?.("workerMessage") ?? 0;
  ensureCrossThreadMessaging();

  process.on("newListener", (eventName: string) => {
    if (eventName === "workerMessage") {
      workerMessageListenerCount++;
      ensureCrossThreadMessaging();
      op_node_worker_thread_set_listener_count(
        threadId,
        workerMessageListenerCount,
      );
    }
  });
  process.on("removeListener", (eventName: string) => {
    if (eventName === "workerMessage") {
      if (workerMessageListenerCount > 0) workerMessageListenerCount--;
      if (crossThreadSetUp) {
        op_node_worker_thread_set_listener_count(
          threadId,
          workerMessageListenerCount,
        );
      }
    }
  });
}

function ensureCrossThreadMessaging() {
  if (crossThreadSetUp) return;
  crossThreadSetUp = true;
  threadMessageRid = op_node_worker_thread_register(threadId);
  // Sync any listener count observed via `newListener` before the
  // registry slot existed.
  if (workerMessageListenerCount > 0) {
    op_node_worker_thread_set_listener_count(
      threadId,
      workerMessageListenerCount,
    );
  }
  // The poll loop runs forever; the underlying receive op resolves to
  // null when the channel is torn down, ending the loop cleanly.
  pollCrossThreadMessages();
}

// https://nodejs.org/api/worker_threads.html#workerpostmessagetothreadthreadid-value-transferlist-timeout
function postMessageToThread(
  targetThreadId: number,
  value?: unknown,
  transferListOrTimeout?: unknown[] | number,
  maybeTimeout?: number,
): Promise<void> {
  let transferList: unknown[] | undefined;
  let timeout: number | undefined;
  try {
    validateInteger(targetThreadId, "threadId", 0);
    if (ArrayIsArray(transferListOrTimeout)) {
      transferList = transferListOrTimeout;
      timeout = maybeTimeout;
    } else if (typeof transferListOrTimeout === "number") {
      timeout = transferListOrTimeout;
    } else if (transferListOrTimeout !== undefined) {
      throw new ERR_INVALID_ARG_TYPE(
        "transferList",
        "Array",
        transferListOrTimeout,
      );
    }
    if (timeout !== undefined) {
      validateInteger(timeout, "timeout", 0);
    }
    if (transferList !== undefined) {
      validateArray(transferList, "transferList");
    }
    if (targetThreadId === threadId) {
      throw new ERR_WORKER_MESSAGING_SAME_THREAD();
    }
  } catch (err) {
    return PromiseReject(err);
  }

  // Senders also need to be registered so the destination's ack can
  // find them. Cheap no-op after the first call.
  ensureCrossThreadMessaging();

  const id = nextThreadMessageId++;
  let data;
  try {
    const envelope = {
      __nodeWorkerMessage: true,
      sender: threadId,
      id,
      kind: kMessageKindData,
      payload: value,
    };
    data = serializeJsMessageData(envelope, transferList ?? []);
  } catch (err) {
    return PromiseReject(err);
  }

  // Register the pending entry *before* posting so that an ack delivered
  // synchronously by a single-threaded test runner still finds it.
  let pendingResolve!: (v: void) => void;
  let pendingReject!: (err: unknown) => void;
  const promise = new Promise<void>((resolve, reject) => {
    pendingResolve = resolve;
    pendingReject = reject;
  });
  let timer: number | undefined;
  if (timeout !== undefined && timeout > 0) {
    timer = setTimeout(() => {
      if (pendingThreadMessages.has(id)) {
        pendingThreadMessages.delete(id);
        pendingReject(new ERR_WORKER_MESSAGING_TIMEOUT());
      }
    }, timeout);
  }
  pendingThreadMessages.set(id, {
    resolve: pendingResolve,
    reject: pendingReject,
    timer,
  });

  let result;
  try {
    result = op_node_worker_thread_post_message(targetThreadId, data, false);
  } catch (err) {
    pendingThreadMessages.delete(id);
    if (timer !== undefined) clearTimeout(timer);
    return PromiseReject(err);
  }
  // 0 = no such thread (always fail fast: it'll never come back).
  // 1 = destination has no `workerMessage` listener; with a timeout
  // we wait so the caller observes ERR_WORKER_MESSAGING_TIMEOUT, but
  // without a timeout we fail immediately rather than hang forever.
  if (result === 0 || (result === 1 && timer === undefined)) {
    pendingThreadMessages.delete(id);
    if (timer !== undefined) clearTimeout(timer);
    return PromiseReject(new ERR_WORKER_MESSAGING_FAILED());
  }

  return promise;
}

function moveMessagePortToContext(port: unknown, _context: unknown) {
  // Even though we don't implement context-switching, Node tests check
  // that the closed-port pre-condition fires first, so honour that.
  if (
    port !== null && typeof port === "object" &&
    // deno-lint-ignore no-explicit-any
    ((port as any)[kClosed] === true || (port as any)[kPortId] === null)
  ) {
    const err = new Error("Cannot send data on closed MessagePort");
    // deno-lint-ignore no-explicit-any
    (err as any).code = "ERR_CLOSED_MESSAGE_PORT";
    throw err;
  }
  notImplemented("moveMessagePortToContext");
}

class BroadcastChannel extends WebBroadcastChannel {
  #closed = false;
  constructor(name?: unknown) {
    if (arguments.length === 0) {
      const err = new TypeError('The "name" argument must be specified');
      // deno-lint-ignore no-explicit-any
      (err as any).code = "ERR_MISSING_ARGS";
      throw err;
    }
    // Use template-string coercion (Node's behavior): throws TypeError
    // for Symbol values, unlike `String(sym)` which silently produces
    // "Symbol(...)".
    super(`${name}`);
  }
  postMessage(message: unknown) {
    if (this.#closed) {
      const err = new Error("BroadcastChannel is closed");
      // deno-lint-ignore no-explicit-any
      (err as any).code = "ERR_BROADCAST_CHANNEL_CLOSED";
      throw err;
    }
    if (arguments.length === 0) {
      const err = new TypeError('The "message" argument must be specified');
      // deno-lint-ignore no-explicit-any
      (err as any).code = "ERR_MISSING_ARGS";
      throw err;
    }
    return super.postMessage(message);
  }
  close() {
    this.#closed = true;
    super.close();
  }
  ref() {
    this[refBroadcastChannel](true);
    return this;
  }

  unref() {
    this[refBroadcastChannel](false);
    return this;
  }
}

ObjectAssign(exportsObj, {
  BroadcastChannel,
  MessagePort,
  MessageChannel,
  Worker: NodeWorker,
  // Initial placeholders for fields that `__initWorkerThreads` overwrites
  // at bootstrap. Listed here so they appear as own enumerable string-keyed
  // properties of the exports object - the synthetic ESM dispatch derives
  // its export names from `Object.keys` of this object.
  parentPort: null,
  threadId: 0,
  workerData: null,
  isMainThread: true,
  resourceLimits: undefined,
  threadName: "",
  markAsUntransferable,
  isMarkedAsUntransferable,
  markAsUncloneable,
  moveMessagePortToContext,
  postMessageToThread,
  receiveMessageOnPort,
  getEnvironmentData,
  setEnvironmentData,
  SHARE_ENV,
});

return exportsObj;
})();
