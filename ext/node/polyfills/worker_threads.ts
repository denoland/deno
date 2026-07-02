// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

(function () {
const { core, internals, primordials } = __bootstrap;
const {
  op_create_worker,
  op_host_get_worker_cpu_usage,
  op_host_post_message,
  op_host_post_message_raw,
  op_host_recv_ctrl,
  op_host_recv_message,
  op_host_recv_message_sync,
  op_host_terminate_worker,
  op_mark_as_untransferable,
  op_message_port_post_message_raw,
  op_message_port_recv_message,
  op_message_port_recv_message_sync,
  op_node_worker_thread_post_message,
  op_node_worker_thread_recv_message,
  op_node_worker_thread_register,
  op_node_worker_thread_set_listener_count,
  op_worker_get_resource_limits,
  op_worker_threads_filename,
} = core.ops;
const {
  deserializeJsMessageData,
  deserializeMessageData,
  isUncloneable,
  markAsUncloneable: webMarkAsUncloneable,
  MessageChannel,
  MessagePort,
  MessagePortIdSymbol,
  MessagePortPrototype,
  MessagePortReceiveMessageOnPortSymbol,
  nodeWorkerThreadCloseCb,
  refMessagePort,
  serializeJsMessageData,
  serializeMessageData,
  unrefParentPort,
} = core.loadExtScript("ext:deno_web/13_message_port.js");
const webidl = core.loadExtScript("ext:deno_webidl/00_webidl.js");
const { notImplemented } = core.loadExtScript("ext:deno_node/_utils.ts");
const {
  ERR_CLOSED_MESSAGE_PORT,
  ERR_CONSTRUCT_CALL_REQUIRED,
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
const { channel: createDiagnosticsChannel } = core.loadExtScript(
  "ext:deno_node/diagnostics_channel.js",
);
const workerThreadsChannel = createDiagnosticsChannel("worker_threads");
const lazyStream = core.createLazyLoader("node:stream");
const {
  BroadcastChannel: WebBroadcastChannel,
  refBroadcastChannel,
} = core.loadExtScript("ext:deno_web/01_broadcast_channel.js");
const { untransferableSymbol } = core.loadExtScript(
  "ext:deno_node/internal_binding/util.ts",
);
const lazyProcess = core.createLazyLoader("node:process");
const lazyUrl = core.createLazyLoader("node:url");
const lazyModule = core.createLazyLoader("node:module");

// Eagerly bind: node:process is in the eager ESM bundle so this is cheap, and
// it's used pervasively throughout this module. `Readable`/`Writable`,
// `fileURLToPath`, and `createRequire` stay deferred via their `lazy*`
// loaders.

const {
  ArrayBufferIsView,
  ArrayIsArray,
  ArrayPrototypeIndexOf,
  ArrayPrototypePush,
  ArrayPrototypeSlice,
  ArrayPrototypeSplice,
  Error,
  EvalError,
  FunctionPrototypeApply,
  FunctionPrototypeBind,
  FunctionPrototypeCall,
  NumberIsFinite,
  NumberIsNaN,
  ObjectAssign,
  ObjectCreate,
  ObjectDefineProperty,
  ObjectHasOwn,
  ObjectKeys,
  ObjectPrototypeIsPrototypeOf,
  Promise,
  PromisePrototypeThen,
  PromiseReject,
  PromiseResolve,
  queueMicrotask,
  RangeError,
  ReferenceError,
  SafeMap,
  SafeRegExp,
  SafeSet,
  String,
  StringPrototypeIndexOf,
  StringPrototypeSlice,
  StringPrototypeSplit,
  StringPrototypeStartsWith,
  StringPrototypeTrim,
  Symbol,
  SymbolAsyncDispose,
  SymbolFor,
  SymbolIterator,
  SyntaxError,
  TypeError,
  URIError,
  Float64Array,
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
    // deno-lint-ignore deno-internal/prefer-primordials no-console
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
  #terminationPromise = undefined;
  #workerOnline = false;
  #exited = false;
  // "RUNNING" | "CLOSED" | "TERMINATING"
  // "TERMINATING" means termination was requested and the close event still
  // needs to be received. "CLOSED" means that we have received a control
  // indicating that the worker is no longer running, but there might still be
  // messages left to receive.
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

    // Ensure the creating thread is wired up to send/receive cross-thread
    // `postMessageToThread` messages. On the main thread under the deferred
    // node bootstrap `__initWorkerThreads` (which normally does this) never
    // runs, so without this the main thread can neither be addressed by
    // `postMessageToThread(0, ...)` nor have its `workerMessage` listeners
    // counted. Idempotent, so the eager paths that already ran it are a
    // no-op here.
    setupCrossThreadMessaging();

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
          if (!lazyProcess().default.allowedNodeEnvironmentFlags.has(flag)) {
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
              !lazyProcess().default.allowedNodeEnvironmentFlags.has(part) ||
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
    // - undefined/null: snapshot current lazyProcess().default.env (isolated copy)
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
      // This also handles passing `lazyProcess().default.env` (a Proxy in Deno) by
      // producing a plain object that can be structured-cloned.
      const envObj = {};
      const keys = ObjectKeys(envOpt);
      for (let i = 0; i < keys.length; i++) {
        envObj[keys[i]] = String(envOpt[keys[i]]);
      }
      env_ = envObj;
    } else if (envOpt !== SHARE_ENV) {
      // Default: snapshot current lazyProcess().default.env so the worker gets an
      // isolated copy, not a live reference to the OS environment.
      // Wrap in try/catch because accessing lazyProcess().default.env requires
      // --allow-env permission in Deno. If unavailable, fall back to
      // shared OS env (env_ stays undefined).
      try {
        const envObj = {};
        const keys = ObjectKeys(lazyProcess().default.env);
        for (let i = 0; i < keys.length; i++) {
          envObj[keys[i]] = lazyProcess().default.env[keys[i]];
        }
        env_ = envObj;
      } catch {
        // No env permission - worker will share the OS environment.
      }
    }
    // When envOpt === SHARE_ENV, env_ stays undefined and the worker
    // will use the default lazyProcess().default.env backed by Deno.env (shared OS env).

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
      execArgv: options?.execArgv ?? [],
      name: this.#name,
      isEval: !!options?.eval,
      isWorkerThread: true,
      hasStdin: !!options?.stdin,
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
      sourceCode = `var __filename = ${
        // deno-lint-ignore deno-internal/prefer-primordials
        JSON.stringify(lazyProcess().default.cwd() + "/[worker eval]")};\n` +
        `var __dirname = ${
          // deno-lint-ignore deno-internal/prefer-primordials
          JSON.stringify(lazyProcess().default.cwd())};\n` +
        `var module = { exports: {} };\n` +
        `var exports = module.exports;\n` +
        code;
      hasSourceCode = true;
      specifier = `data:text/javascript,`;
    } else if (
      !(typeof specifier === "object" && specifier.protocol === "data:")
    ) {
      // deno-lint-ignore deno-internal/prefer-primordials
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
        // deno-lint-ignore deno-internal/prefer-primordials
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
    lazyProcess().default.nextTick(() =>
      lazyProcess().default.emit("worker", this)
    );

    if (workerThreadsChannel.hasSubscribers) {
      workerThreadsChannel.publish({ worker: this });
    }
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

      const isTerminating = this.#status === "TERMINATING";

      switch (type) {
        case 1: { // TerminalError
          this.#status = "CLOSED";
          if (!isTerminating && this.listenerCount("error") > 0) {
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
          // Drain pending messages before closing stdio and emitting exit:
          // any stdio chunks still queued on the message channel must be
          // pushed onto the Readable streams *before* we EOF them, otherwise
          // we hit "stream.push() after EOF" if the Close control arrives
          // before the last stdout/stderr message (Node.js behavior).
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
          // Drain pending messages before closing stdio and emitting exit:
          // any stdio chunks still queued on the message channel must be
          // pushed onto the Readable streams *before* we EOF them, otherwise
          // we hit "stream.push() after EOF" if the Close control arrives
          // before the last stdout/stderr message (Node.js behavior).
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
    while (this.#status !== "TERMINATING") {
      this.#messagePromise = op_host_recv_message(this.#id);
      if (!this.#refed) {
        core.unrefOpPromise(this.#messagePromise);
      }
      const data = await this.#messagePromise;
      if (this.#status === "TERMINATING" || data === null) {
        return;
      }
      if (!this.#dispatchWorkerThreadMessage(data)) return;
      // Drain messages already queued on the host side instead of taking the
      // async op + Promise path for each. The whole burst is processed within
      // this event-loop turn; the batch limit prevents starving the event loop
      // under a sustained flood.
      for (
        let i = 0;
        i < 1000 && this.#status !== "TERMINATING";
        i++
      ) {
        const syncData = op_host_recv_message_sync(this.#id);
        if (syncData === null) break;
        // Each message dispatch is its own task. Yield a microtask before
        // delivering this already-dequeued message so a handler that re-armed
        // itself in a microtask after the previous dispatch (e.g. an
        // `events.once` listener that re-attaches in a `.then`) is installed
        // first -- otherwise the message reaches the stale handler and is
        // lost. A synchronous checkpoint can't help: V8 won't run microtasks
        // reentrantly while we are already inside one.
        await new Promise((resolve) => queueMicrotask(() => resolve()));
        if (this.#status === "TERMINATING") {
          return;
        }
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
      // Reject non-serializable values (e.g. URL) and per-instance
      // markAsUncloneable values before V8's serializer silently turns them
      // into `{}`, matching the web MessagePort path and Node's behavior.
      if (isUncloneable(message)) {
        throw new DOMException(
          "Cannot clone object of unsupported type.",
          "DataCloneError",
        );
      }
      op_host_post_message_raw(
        this.#id,
        serializeMessageData(message),
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
    if (this.#status === "CLOSED") {
      return PromiseResolve(undefined);
    }
    if (this.#terminationPromise !== undefined) {
      return this.#terminationPromise;
    }

    this.#terminationPromise = new Promise((resolve) => {
      this.once("exit", resolve);
    });

    this.#status = "TERMINATING";
    if (this.#controlPromise) {
      core.refOpPromise(this.#controlPromise);
    }
    if (this.#messagePromise) {
      core.refOpPromise(this.#messagePromise);
    }
    op_host_terminate_worker(this.#id);
    return this.#terminationPromise;
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

    // Per-event-name listener map so the same listener reference
    // passed to parentPort.on twice for the same event registers only
    // once, matching Node MessagePort behavior. Maps are created on
    // demand since parentPort accepts any event name.
    // deno-lint-ignore no-explicit-any
    type ListenerMap = SafeMap<(...args: any[]) => void, (ev: any) => any>;
    const parentPortListeners: Record<string, ListenerMap> = ObjectCreate(null);
    const getParentPortListenerMap = (name: string): ListenerMap => {
      let map = parentPortListeners[name];
      if (map === undefined) {
        map = new SafeMap();
        parentPortListeners[name] = map;
      }
      return map;
    };

    // Create parentPort as a separate object that delegates to the web
    // worker's native APIs. We capture the native methods here (before
    // user code runs) so that user code overriding globalThis.postMessage
    // (e.g. Emscripten/z3-solver) doesn't cause infinite recursion.
    const nativePostMessage = FunctionPrototypeBind(
      globalThis.postMessage,
      globalThis,
    );
    const nativeAddEventListener = FunctionPrototypeBind(
      globalThis.addEventListener,
      globalThis,
    );
    const nativeRemoveEventListener = FunctionPrototypeBind(
      globalThis.removeEventListener,
      globalThis,
    );
    // Track message listener count to prevent double delivery.
    // When parentPort message listeners exist, suppress the web IDL
    // onmessage handler (globalThis.onmessage) since both would fire
    // for the same MessageEvent.
    let messageListenerCount = 0;
    // Track Web API message listeners to prevent underflow on
    // double-remove (same concern as the Node-style off() path).
    const webMessageListeners = new SafeSet();

    parentPort = ObjectCreate(null) as ParentPort;
    parentPort.postMessage = function (message, transferOrOptions?) {
      return nativePostMessage(message, transferOrOptions);
    };
    parentPort.addEventListener = function (name, listener, options?) {
      nativeAddEventListener(name, listener, options);
      if (name === "message" && !webMessageListeners.has(listener)) {
        webMessageListeners.add(listener);
        messageListenerCount++;
      }
    };
    parentPort.removeEventListener = function (name, listener) {
      nativeRemoveEventListener(name, listener);
      if (name === "message" && webMessageListeners.has(listener)) {
        webMessageListeners.delete(listener);
        messageListenerCount--;
      }
    };
    // Delegate parentPort.onmessage to globalThis.onmessage so that
    // setting parentPort.onmessage = handler works like the old code
    // where parentPort === globalThis.
    ObjectDefineProperty(parentPort, "onmessage", {
      __proto__: null,
      get() {
        return globalThis.onmessage;
      },
      set(handler) {
        globalThis.onmessage = handler;
      },
      configurable: true,
      enumerable: true,
    });

    // Only intercept globalThis.onmessage for Node worker threads
    // (not plain Deno web workers) to prevent double message delivery
    // when both parentPort.on('message') and self.onmessage are set.
    if (maybeWorkerMetadata) {
      let storedOnmessage: ((ev: Event) => void) | null = null;
      // Dynamically add/remove the forwarding listener so we don't
      // keep a permanent "message" listener on globalThis. A permanent
      // listener would make hasMessageEventListener() always true and
      // prevent the worker from exiting.
      let onmessageForwarder: ((ev: Event) => void) | null = null;
      ObjectDefineProperty(globalThis, "onmessage", {
        __proto__: null,
        get() {
          return storedOnmessage;
        },
        set(handler) {
          // Remove old forwarder if any
          if (onmessageForwarder) {
            nativeRemoveEventListener("message", onmessageForwarder);
            onmessageForwarder = null;
          }
          storedOnmessage = handler;
          // Add forwarder only when a handler is set
          if (typeof handler === "function") {
            onmessageForwarder = (ev: Event) => {
              if (messageListenerCount > 0) return;
              if (typeof storedOnmessage === "function") {
                storedOnmessage(ev);
              }
            };
            nativeAddEventListener("message", onmessageForwarder);
          }
        },
        configurable: true,
        enumerable: true,
      });
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
        lazyProcess().default.env = env;
      }

      // Get resolved resource limits from the Rust side (includes V8
      // defaults for unspecified fields), matching Node.js behavior.
      const resolvedLimits = op_worker_get_resource_limits();
      if (resolvedLimits) {
        resourceLimits = resolvedLimits;
      } else {
        resourceLimits = {};
      }

      // Set lazyProcess().default.argv for worker threads.
      // In Node.js, worker lazyProcess().default.argv is [execPath, scriptPath, ...argv].
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
        lazyProcess().default.argv = [
          lazyProcess().default.execPath,
          scriptPath,
        ];
        if (metadata.argv) {
          for (let i = 0; i < metadata.argv.length; i++) {
            lazyProcess().default.argv[i + 2] = metadata.argv[i];
          }
        }

        // Set lazyProcess().default.execArgv for worker threads.
        if (metadata.execArgv) {
          lazyProcess().default.execArgv = metadata.execArgv;
          core.loadExtScript(
            "ext:deno_node/internal_binding/node_options.ts",
          ).setOptionSourceExecArgv(metadata.execArgv);
          for (let i = 0; i < metadata.execArgv.length; i++) {
            if (metadata.execArgv[i] === "--trace-warnings") {
              lazyProcess().default.traceProcessWarnings = true;
            }
          }
        }

        // Replace lazyProcess().default.stdin with a Readable that receives
        // data from the parent via WORKER_STDIN messages.
        if (metadata.hasStdin) {
          const workerStdin = new (lazyStream().Readable)({ read() {} });
          lazyProcess().default.stdin = workerStdin;

          // Register an early listener to intercept stdin messages
          // before any user-registered handlers. Remove the listener
          // once stdin ends so the worker can exit cleanly.
          const stdinHandler = (ev) => {
            const msg = ev.data;
            if (isWorkerStdinMsg(msg)) {
              // deno-lint-ignore deno-internal/prefer-primordials
              workerStdin.push(msg.data);
              ev.stopImmediatePropagation();
            } else if (isWorkerStdinEndMsg(msg)) {
              // deno-lint-ignore deno-internal/prefer-primordials
              workerStdin.push(null);
              parentPort.removeEventListener("message", stdinHandler);
              ev.stopImmediatePropagation();
            }
          };
          parentPort.addEventListener("message", stdinHandler);
        }

        // Forward stdout writes to the parent so worker.stdout
        // is readable from the host side.
        const origStdoutWrite = FunctionPrototypeBind(
          lazyProcess().default.stdout.write,
          lazyProcess().default.stdout,
        );
        lazyProcess().default.stdout.write = function (
          chunk,
          encoding,
          callback,
        ) {
          parentPort.postMessage({
            type: "WORKER_STDOUT",
            data: chunk,
          });
          return FunctionPrototypeCall(
            origStdoutWrite,
            lazyProcess().default.stdout,
            chunk,
            encoding,
            callback,
          );
        };

        // Forward stderr writes to the parent so worker.stderr
        // is readable from the host side.
        const origStderrWrite = FunctionPrototypeBind(
          lazyProcess().default.stderr.write,
          lazyProcess().default.stderr,
        );
        lazyProcess().default.stderr.write = function (
          chunk,
          encoding,
          callback,
        ) {
          parentPort.postMessage({
            type: "WORKER_STDERR",
            data: chunk,
          });
          return FunctionPrototypeCall(
            origStderrWrite,
            lazyProcess().default.stderr,
            chunk,
            encoding,
            callback,
          );
        };
      }
    }
    patchMessagePortIfFound(workerData);

    parentPort.off = parentPort.removeListener = function (
      name,
      listener,
    ) {
      const map = parentPortListeners[name];
      if (map !== undefined) {
        const wrapper = map.get(listener);
        if (wrapper !== undefined) {
          nativeRemoveEventListener(name, wrapper);
          map.delete(listener);
          if (name === "message") messageListenerCount--;
        }
      }
      return parentPort;
    };
    parentPort.on = parentPort.addListener = function (
      name,
      listener,
    ) {
      const map = getParentPortListenerMap(name);
      if (map.has(listener)) {
        // Same listener already registered for this event -- dedup to
        // match Node MessagePort behavior.
        return parentPort;
      }
      // deno-lint-ignore no-explicit-any
      const _listener = (ev: any) => {
        const message = ev.data;
        patchMessagePortIfFound(message);
        return listener(message);
      };
      map.set(listener, _listener);
      nativeAddEventListener(name, _listener);
      if (name === "message") messageListenerCount++;
      return parentPort;
    };

    parentPort.once = function (name, listener) {
      const map = getParentPortListenerMap(name);
      if (map.has(listener)) {
        return parentPort;
      }
      // deno-lint-ignore no-explicit-any
      const _listener = (ev: any) => {
        map.delete(listener);
        if (name === "message") messageListenerCount--;
        const message = ev.data;
        patchMessagePortIfFound(message);
        return listener(message);
      };
      map.set(listener, _listener);
      nativeAddEventListener(name, _listener, { once: true });
      if (name === "message") messageListenerCount++;
      return parentPort;
    };

    // mocks
    parentPort.setMaxListeners = () => {};
    parentPort.getMaxListeners = () => Infinity;
    parentPort.eventNames = () => [""];
    parentPort.listenerCount = () => 0;

    parentPort.emit = () => notImplemented("parentPort.emit");
    parentPort.removeAllListeners = () =>
      notImplemented("parentPort.removeAllListeners");

    nativeAddEventListener("offline", () => {
      parentPort.emit("close");
    });
    parentPort.unref = () => {
      parentPort[unrefParentPort] = true;
      // Also set on globalThis so runtime/js/99_main.js event loop
      // check (globalThis[unrefParentPort]) still works.
      globalThis[unrefParentPort] = true;
    };
    parentPort.ref = () => {
      parentPort[unrefParentPort] = false;
      globalThis[unrefParentPort] = false;
    };

    if (isWorkerThread) {
      // Notify the host that the worker is online
      parentPort.postMessage(
        {
          type: "WORKER_ONLINE",
        } satisfies WorkerOnlineMsg,
      );
    }
  }

  // In Node, `globalThis.MessageChannel` / `globalThis.MessagePort` ARE
  // the `worker_threads` versions (so port instances have
  // `.on`/`.off`/`.emit`/etc., and `worker_threads.MessagePort` is
  // identity-equal to the global `MessagePort`). Tests in the
  // node_compat suite import individual helpers from `worker_threads`
  // and then use `MessageChannel` from the global scope, expecting
  // Node-style ports. Override the global symbols once at bootstrap so
  // this pattern works. `instanceof MessageChannel` / `instanceof
  // MessagePort` still work either way because both Node wrappers
  // route through the web prototype chain.
  try {
    ObjectDefineProperty(globalThis, "MessageChannel", {
      __proto__: null,
      value: NodeMessageChannel,
      writable: true,
      enumerable: false,
      configurable: true,
    });
    ObjectDefineProperty(globalThis, "MessagePort", {
      __proto__: null,
      value: NodeMessagePort,
      writable: true,
      enumerable: false,
      configurable: true,
    });
  } catch {
    // globalThis may be a sandbox without writable property descriptors;
    // ignore.
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
let crossThreadMessagingSetUp = false;
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
    let status = kAckStatusOk;
    try {
      lazyProcess().default.emit(
        "workerMessage",
        envelope.payload,
        envelope.sender,
      );
    } catch {
      status = kAckStatusError;
    }
    sendAck(envelope.sender, envelope.id, status);
  }
}

// Eagerly observe listener add/remove for `workerMessage` so the count
// is always in sync. Actual registration of the cross-thread resource
// is deferred to `ensureCrossThreadMessaging` so threads that never
// touch `postMessageToThread` don't carry an extra entry in
// `core.resources()` (which several finalization tests assert on).
function setupCrossThreadMessaging() {
  // Idempotent: under the deferred node bootstrap this is invoked lazily
  // from the `Worker` constructor on the main thread (where `initialize()`
  // -- and thus `__initWorkerThreads` -- never auto-runs), as well as
  // eagerly from `__initWorkerThreads` on workers and the eager main path.
  // Installing the `newListener` hook twice would double-count listeners.
  if (crossThreadMessagingSetUp) return;
  crossThreadMessagingSetUp = true;
  workerMessageListenerCount =
    lazyProcess().default.listenerCount?.("workerMessage") ?? 0;
  if (workerMessageListenerCount > 0) {
    ensureCrossThreadMessaging();
  }

  lazyProcess().default.on("newListener", (eventName: string) => {
    if (eventName === "workerMessage") {
      workerMessageListenerCount++;
      ensureCrossThreadMessaging();
      op_node_worker_thread_set_listener_count(
        threadId,
        workerMessageListenerCount,
      );
    }
  });
  lazyProcess().default.on("removeListener", (eventName: string) => {
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
  // 0 = no such thread, 1 = destination has no `workerMessage` listener.
  if (result === 0 || result === 1) {
    pendingThreadMessages.delete(id);
    if (timer !== undefined) clearTimeout(timer);
    return PromiseReject(new ERR_WORKER_MESSAGING_FAILED());
  }

  return promise;
}

function markAsUntransferable(obj: object) {
  // Primitives are silently ignored to match Node.
  if (obj === null) return;
  const t = typeof obj;
  if (t !== "object" && t !== "function") return;

  if (core.isArrayBuffer(obj)) {
    // Sets V8's detach key so postMessage(..., [ab]) throws DataCloneError
    // without detaching the buffer.
    op_mark_as_untransferable(obj as ArrayBuffer);
  }

  // For non-ArrayBuffer transferables (e.g. MessagePort) Node uses a
  // symbol marker that postMessage checks before adding to the transfer
  // list; we also set it on ArrayBuffer so `isMarkedAsUntransferable`
  // returns true without needing to poke at V8's detach key from JS.
  ObjectDefineProperty(obj, untransferableSymbol, {
    __proto__: null,
    value: true,
    enumerable: false,
    writable: false,
    configurable: false,
  });
}

function isMarkedAsUntransferable(obj: unknown): boolean {
  if (obj === null) return false;
  const t = typeof obj;
  if (t !== "object" && t !== "function") return false;
  // Check own property -- Node's spec is explicit that the mark is *not*
  // inherited through the prototype chain.
  return ObjectHasOwn(obj as object, untransferableSymbol) &&
    (obj as Record<symbol, unknown>)[untransferableSymbol] === true;
}

function markAsUncloneable(obj: unknown) {
  return webMarkAsUncloneable(obj);
}
const lazyVm = () => core.loadExtScript("ext:deno_node/vm.js");

// Move a MessagePort into a vm.Context. The returned object lives in the
// target context (so its prototype chain is in that realm and it is *not*
// `instanceof Object` in the calling realm). Messages arriving on the
// underlying port are deserialized without the global host-object
// deserializers, mirroring Node's behavior where the target context lacks
// the JS classes registered in the source realm: any host object (e.g. a
// crypto KeyObject) triggers `messageerror` with the
// `ERR_MESSAGE_TARGET_CONTEXT_UNAVAILABLE` code, while plain transferable
// data is delivered as `message`.
function moveMessagePortToContext(
  port: MessagePort,
  context: object,
): object {
  if (!(ObjectPrototypeIsPrototypeOf(MessagePortPrototype, port))) {
    throw new ERR_INVALID_ARG_TYPE("port", "MessagePort", port);
  }
  // Node checks closed-port state before vm.Context to give a clearer
  // error when the port is detached -- order matters for tests in the
  // node_compat suite that pass an empty {} as the context.
  const portId = port[MessagePortIdSymbol];
  if (portId === null) {
    throw new ERR_CLOSED_MESSAGE_PORT();
  }
  const vm = lazyVm();
  if (!vm.isContext(context)) {
    throw new ERR_INVALID_ARG_TYPE("context", "vm.Context", context);
  }
  // Take ownership of the port: clear the id on the original so it can no
  // longer be used from this context.
  port[MessagePortIdSymbol] = null;

  // Allocate the wrapper inside the target context so its prototype chain
  // is the target realm's (i.e., `wrapper instanceof Object` in the caller
  // realm is false, matching Node).
  const wrapper = vm.runInContext("({})", context);
  wrapper.onmessage = null;
  wrapper.onmessageerror = null;

  let enabled = false;
  let closed = false;

  const dispatchMessageError = (err: object) => {
    if (typeof wrapper.onmessageerror === "function") {
      try {
        wrapper.onmessageerror({ data: err });
      } catch {
        // Silently ignore - user handler errors must not break the loop.
      }
    }
  };

  const dispatchMessage = (msg: unknown) => {
    if (typeof wrapper.onmessage === "function") {
      try {
        wrapper.onmessage({ data: msg });
      } catch {
        // Silently ignore - user handler errors must not break the loop.
      }
    }
  };

  wrapper.start = () => {
    if (enabled || closed) return;
    enabled = true;
    (async () => {
      while (!closed) {
        let data;
        try {
          data = await op_message_port_recv_message(portId);
        } catch {
          break;
        }
        if (data === null) break;
        // Intentionally deserialize without the host-object deserializers
        // registry. Any host object marker in the stream throws, which we
        // surface as `messageerror` per Node semantics.
        let message;
        try {
          // `data` is either the raw serialized buffer (no transferables) or a
          // `{ data, transferables }` object. Deserialize without the
          // host-object deserializers registry either way.
          message = deserializeMessageData(
            ArrayBufferIsView(data) ? data : data.data,
            false,
          );
        } catch {
          const err = new Error(
            "Message could not be deserialized in the target context",
          );
          // deno-lint-ignore no-explicit-any
          (err as any).code = "ERR_MESSAGE_TARGET_CONTEXT_UNAVAILABLE";
          dispatchMessageError(err);
          continue;
        }
        dispatchMessage(message);
      }
    })();
  };

  wrapper.close = () => {
    if (closed) return;
    closed = true;
    try {
      core.close(portId);
    } catch {
      // ignore
    }
  };

  wrapper.postMessage = (msg: unknown) => {
    if (closed) return;
    op_message_port_post_message_raw(portId, serializeMessageData(msg));
  };

  return wrapper;
}

/**
 * @param { MessagePort } port
 * @returns {object | undefined}
 */
function receiveMessageOnPort(port: MessagePort): object | undefined {
  if (!(ObjectPrototypeIsPrototypeOf(MessagePortPrototype, port))) {
    const err = new TypeError(
      'The "port" argument must be a MessagePort instance',
    );
    err["code"] = "ERR_INVALID_ARG_TYPE";
    throw err;
  }
  port[MessagePortReceiveMessageOnPortSymbol] = true;
  const data = op_message_port_recv_message_sync(port[MessagePortIdSymbol]);
  if (data === null) return undefined;
  const message = deserializeJsMessageData(data)[0];
  patchMessagePortIfFound(message);
  return { message };
}

// Implemented as a function (not a class) so calling without `new` throws
// our Node-style `ERR_CONSTRUCT_CALL_REQUIRED` instead of V8's default
// "Class constructor X cannot be invoked without 'new'" -- the node_compat
// suite asserts the specific code/constructor for both forms.
// deno-lint-ignore no-explicit-any
function NodeMessageChannel(this: any) {
  if (new.target === undefined) {
    throw new ERR_CONSTRUCT_CALL_REQUIRED("MessageChannel");
  }
  {
    const channel = new MessageChannel();
    const port1 = webMessagePortToNodeMessagePort(channel.port1);
    const port2 = webMessagePortToNodeMessagePort(channel.port2);
    // The web MessageChannel.prototype defines port1/port2 as getter-only
    // accessors, so a plain `this.port1 = ...` triggers a setter trap. Use
    // defineProperty to shadow the prototype accessors with own data
    // properties that hold our Node-style ports.
    ObjectDefineProperty(this, "port1", {
      __proto__: null,
      value: port1,
      writable: true,
      enumerable: true,
      configurable: true,
    });
    ObjectDefineProperty(this, "port2", {
      __proto__: null,
      value: port2,
      writable: true,
      enumerable: true,
      configurable: true,
    });

    // When one port is closed, force the paired port to start its recv
    // loop so it drains any in-flight messages and observes the channel
    // tear-down via its `end-of-stream` -> closeCb path. This also makes
    // sure the paired port's close event fires even if no message
    // listener was attached.
    const origClose1 = FunctionPrototypeBind(port1.close, port1);
    const origClose2 = FunctionPrototypeBind(port2.close, port2);

    port1.close = (cb) => {
      origClose1(cb);
      try {
        port2.start();
      } catch {
        // start may throw if the port is already detached -- safe to
        // ignore.
      }
    };
    port2.close = (cb) => {
      origClose2(cb);
      try {
        port1.start();
      } catch {
        // see above
      }
    };
  }
}

const nodePortListenersSymbol = Symbol("nodePortListeners");
// Reuse the web MessageChannel's prototype so `instanceof MessageChannel`
// still works regardless of whether the user got the global MessageChannel
// (NodeMessageChannel) or the web one. Port1/port2 own properties on each
// instance shadow the prototype's getters.
// deno-lint-ignore no-explicit-any
(NodeMessageChannel as any).prototype = MessageChannel.prototype;
function webMessagePortToNodeMessagePort(port: MessagePort) {
  // Patch idempotently: a port can be reached more than once through
  // patchMessagePortIfFound (e.g. when included in a nested message
  // payload). Re-patching would discard tracked listeners.
  // deno-lint-ignore no-explicit-any
  if ((port as any)[nodePortListenersSymbol]) return port;
  // Track listeners per port and per event name so the same handler
  // reference passed to `on` twice registers only once (Node parity),
  // and `off` can locate the wrapper that was actually attached.
  //
  // Maps are created on demand: Node's MessagePort, like EventEmitter,
  // accepts arbitrary event names so `port.on('foo', fn) + port.emit('foo')`
  // works. The Web-style events (`message`, `messageerror`, `close`) are
  // routed through addEventListener; everything else falls through to a
  // plain EventEmitter-like dispatch that runs the registered functions
  // with whatever args `emit` was given.
  // deno-lint-ignore no-explicit-any
  type ListenerMap = SafeMap<(...args: any[]) => void, (ev: any) => any>;
  const portListeners: Record<string | symbol, ListenerMap> = ObjectCreate(
    null,
  );
  const getListenerMap = (name: string | symbol): ListenerMap => {
    let map = portListeners[name as string];
    if (map === undefined) {
      map = new SafeMap();
      portListeners[name as string] = map;
    }
    return map;
  };
  // Listener bags for arbitrary event names (anything other than the three
  // Web event names) -- they don't go through EventTarget's dispatch path.
  const customEventListeners = new SafeMap<
    string | symbol,
    // deno-lint-ignore no-explicit-any
    Array<(...a: any[]) => void>
  >();
  const isWebEvent = (name: string | symbol): boolean =>
    name === "message" || name === "messageerror" || name === "close";
  ObjectDefineProperty(port, nodePortListenersSymbol, {
    __proto__: null,
    value: portListeners,
    writable: false,
    enumerable: false,
    configurable: false,
  });
  port.on = port.addListener = function (this: MessagePort, name, listener) {
    if (!isWebEvent(name)) {
      // EventEmitter-like dispatch path for arbitrary event names.
      // `port.on('foo', fn) + port.emit('foo', 'bar')` works like Node's
      // MessagePort, which mixes in EventEmitter semantics on top of the
      // Web EventTarget interface.
      let arr = customEventListeners.get(name);
      if (arr === undefined) {
        arr = [];
        customEventListeners.set(name, arr);
      }
      ArrayPrototypePush(arr, listener);
      return this;
    }
    const map = getListenerMap(name);
    if (map.has(listener)) {
      // Same listener already registered for this event on this port --
      // matches Node.js MessagePort, where repeated `.on('message', fn)`
      // calls with the same reference do not produce duplicate
      // deliveries.
      return this;
    }
    // deno-lint-ignore no-explicit-any
    const _listener = (ev: any) => {
      patchMessagePortIfFound(ev.data);
      listener(ev.data);
    };
    map.set(listener, _listener);
    port.addEventListener(name, _listener);
    if (name === "message") {
      // Mirror the auto-start behavior of `port.onmessage = fn` so Node
      // code that does `port.on('message', ...)` without an explicit
      // `port.start()` still receives messages. Deferred via a
      // microtask so `receiveMessageOnPort` (sync) gets a chance to
      // drain the queue first -- this is the same pattern the web
      // MessagePort applies in its `message` event handler init, which
      // `npm:piscina` relies on. Messages buffered before the listener
      // was attached are still picked up by the next iteration of the
      // recv loop (or by `MessagePort.close()`'s drain when the paired
      // port closes in the same turn).
      PromisePrototypeThen(PromiseResolve(undefined), () => port.start());
    }
    return this;
  };
  port.off = port.removeListener = function (
    this: MessagePort,
    name,
    listener,
  ) {
    if (!isWebEvent(name)) {
      const arr = customEventListeners.get(name);
      if (arr !== undefined) {
        const idx = ArrayPrototypeIndexOf(arr, listener);
        if (idx !== -1) ArrayPrototypeSplice(arr, idx, 1);
      }
      return this;
    }
    const map = getListenerMap(name);
    const wrapper = map.get(listener);
    if (wrapper !== undefined) {
      port.removeEventListener(name, wrapper);
      map.delete(listener);
    }
    return this;
  };
  // Node's MessagePort mixes in `emit` from EventEmitter so calling
  // `port.emit('foo', arg)` invokes listeners registered via `port.on`
  // *and* dispatches the equivalent Event for addEventListener listeners.
  // deno-lint-ignore no-explicit-any
  port.emit = function (this: MessagePort, name, ...args: any[]) {
    // Dispatch an Event so addEventListener listeners fire too. Use a
    // minimal Event with `detail` copied from the first arg to match the
    // common Node pattern of `emit('foo', payload)`.
    const ev = new Event(String(name));
    if (args.length > 0) {
      // deno-lint-ignore no-explicit-any
      (ev as any).detail = args[0];
    }
    port.dispatchEvent(ev);
    // Fire EventEmitter-style listeners.
    const arr = customEventListeners.get(name);
    if (arr !== undefined) {
      // Iterate over a copy so handlers that remove themselves don't
      // skip the next listener.
      const copy = ArrayPrototypeSlice(arr);
      for (let i = 0; i < copy.length; i++) {
        try {
          FunctionPrototypeApply(copy[i], undefined, args);
        } catch {
          // EventEmitter semantics swallow handler exceptions when
          // there's no 'error' listener; we just continue to the next
          // handler.
        }
      }
    }
    return arr !== undefined && arr.length > 0;
  };
  port[nodeWorkerThreadCloseCb] = () => {
    // Dispatch asynchronously so listeners attached via `port.on('close', cb)`
    // (or via `port.close(cb)` which goes through addEventListener) see the
    // event after the current synchronous turn. This matches Node's
    // MessagePort, where close events fire on the next tick.
    queueMicrotask(() => {
      port[refMessagePort](false);
      // Mark the port as detached so `util.inspect(port)` shows
      // `active: false` after the close event has fired. The underlying
      // resource may already be closed (recv loop saw end-of-stream);
      // swallow any "bad resource id" error from a redundant core.close.
      if (port[MessagePortIdSymbol] !== null) {
        try {
          core.close(port[MessagePortIdSymbol]);
        } catch {
          // already closed
        }
        port[MessagePortIdSymbol] = null;
      }
      port.dispatchEvent(new Event("close"));
    });
  };
  port.unref = () => {
    port[refMessagePort](false);
  };
  port.ref = () => {
    port[refMessagePort](true);
  };
  const webPostMessage = port.postMessage;
  port.postMessage = function postMessage(message, transferOrOptions) {
    // Node-style validation + untransferable check on the second argument.
    // The Web spec normalizes via WebIDL dictionary conversion (different
    // error wording), so do the validation up-front and pass a normalized
    // value through. For iterables we materialize into an array so the
    // untransferable check below doesn't consume the iterator before the
    // serializer sees it.
    const normalized = transferOrOptions;
    if (arguments.length >= 2 && transferOrOptions !== undefined) {
      if (transferOrOptions === null) {
        // null is allowed; treated as no transferables.
      } else if (
        typeof transferOrOptions !== "object" &&
        typeof transferOrOptions !== "function"
      ) {
        const e = new TypeError(
          "Optional transferList argument must be an iterable",
        );
        // deno-lint-ignore no-explicit-any
        (e as any).code = "ERR_INVALID_ARG_TYPE";
        throw e;
      } else if (transferOrOptions[SymbolIterator] !== undefined) {
        // Only check untransferable markers when we can do so without
        // consuming the iterator -- a user-provided iterator can only
        // be drained once and we have to leave that drain to the
        // underlying web `postMessage`. Arrays are random-access so
        // we can inspect them without consuming.
        if (ArrayIsArray(transferOrOptions)) {
          for (let i = 0; i < transferOrOptions.length; i++) {
            const item = transferOrOptions[i];
            if (
              item !== null && typeof item === "object" &&
              item[untransferableSymbol] === true
            ) {
              throw new DOMException(
                "Value not transferable",
                "DataCloneError",
              );
            }
          }
        }
        // For non-array iterables, pass through unchanged so the
        // serializer below sees the original iterator.
      } else {
        // Treat as a StructuredSerializeOptions dict. Only `transfer` is
        // recognized; if present, validate that it's iterable.
        const t = transferOrOptions.transfer;
        if (t !== undefined) {
          if (
            t === null ||
            (typeof t !== "object" && typeof t !== "function") ||
            t[SymbolIterator] === undefined ||
            (typeof t[SymbolIterator] === "function" &&
              typeof t[SymbolIterator]().next !== "function")
          ) {
            const e = new TypeError(
              "Optional options.transfer argument must be an iterable",
            );
            // deno-lint-ignore no-explicit-any
            (e as any).code = "ERR_INVALID_ARG_TYPE";
            throw e;
          }
          // Inspect arrays for untransferable markers. For non-array
          // iterables we have to leave the drain to the serializer
          // below -- consuming the iterator here would either lose
          // values or, for an infinite generator (the
          // terminate-transfer-list test), short-circuit Node's
          // expected hang.
          if (ArrayIsArray(t)) {
            for (let i = 0; i < t.length; i++) {
              const item = t[i];
              if (
                item !== null && typeof item === "object" &&
                item[untransferableSymbol] === true
              ) {
                throw new DOMException(
                  "Value not transferable",
                  "DataCloneError",
                );
              }
            }
          }
        }
      }
    }

    return FunctionPrototypeCall(
      webPostMessage,
      port,
      message,
      normalized,
    );
  };
  port.once = (name: string | symbol, listener) => {
    const fn = (event) => {
      port.off(name, fn);
      return listener(event);
    };
    port.on(name, fn);
  };
  return port;
}

// TODO(@marvinhagemeister): Recursively iterating over all message
// properties seems slow.
// Maybe there is a way we can patch the prototype of MessagePort _only_
// inside worker_threads? For now correctness is more important than perf.
// deno-lint-ignore no-explicit-any
function patchMessagePortIfFound(data: any, seen = new SafeSet<any>()) {
  if (data === null || typeof data !== "object" || seen.has(data)) {
    return;
  }
  seen.add(data);

  if (ObjectPrototypeIsPrototypeOf(MessagePortPrototype, data)) {
    webMessagePortToNodeMessagePort(data);
  } else {
    for (const obj in data as Record<string, unknown>) {
      if (ObjectHasOwn(data, obj)) {
        patchMessagePortIfFound(data[obj], seen);
      }
    }
  }
}

class BroadcastChannel extends WebBroadcastChannel {
  ref() {
    this[refBroadcastChannel](true);
    return this;
  }

  unref() {
    this[refBroadcastChannel](false);
    return this;
  }
}

const { locks } = core.createLazyLoader("ext:deno_web/locks.js")();

// Node's `worker_threads.MessagePort` is a function (not a class) that throws
// `ERR_CONSTRUCT_CALL_INVALID` whether called as `MessagePort()` or
// `new MessagePort()`. Mirror that here while keeping
// `port instanceof MessagePort` and `port.constructor === MessagePort` true
// by routing through the web MessagePort's prototype. Named "MessagePort"
// so that `port.constructor.name === "MessagePort"` and the default
// inspect output reads as MessagePort, not the wrapper.
// deno-lint-ignore no-explicit-any
const NodeMessagePort: any = {
  MessagePort: function MessagePort() {
    const err = new TypeError(
      "Constructor for class MessagePort cannot be invoked",
    );
    // deno-lint-ignore no-explicit-any
    (err as any).code = "ERR_CONSTRUCT_CALL_INVALID";
    throw err;
  },
}.MessagePort;
NodeMessagePort.prototype = MessagePort.prototype;
ObjectDefineProperty(MessagePort.prototype, "constructor", {
  __proto__: null,
  value: NodeMessagePort,
  writable: true,
  enumerable: false,
  configurable: true,
});

ObjectAssign(exportsObj, {
  BroadcastChannel,
  MessagePort: NodeMessagePort,
  MessageChannel: NodeMessageChannel,
  Worker: NodeWorker,
  // Initial placeholders for fields that `__initWorkerThreads` overwrites
  // at bootstrap. Listed here so they appear as own enumerable string-keyed
  // properties of the exports object - the synthetic ESM dispatch derives
  // its export names from `Object.keys` of this object.
  parentPort: null,
  threadId: 0,
  workerData: null,
  isMainThread: true,
  // Deno has no internal Node worker threads (e.g. module loader threads),
  // so this is always false in the main thread and user-created workers.
  isInternalThread: false,
  // Main-thread default ({}). `__initWorkerThreads` overwrites it (with the
  // worker's resolved limits in a worker). Under node-defer, if that init never
  // runs for a worker_threads-only main program, this default still satisfies
  // Node's contract (an empty resourceLimits on the main thread).
  resourceLimits: {},
  threadName: "",
  locks,
  markAsUncloneable,
  markAsUntransferable,
  isMarkedAsUntransferable,
  moveMessagePortToContext,
  postMessageToThread,
  receiveMessageOnPort,
  getEnvironmentData,
  setEnvironmentData,
  SHARE_ENV,
});

// node-defer: under deferred bootstrap, `__initWorkerThreads` may never run on
// the main thread (it ran from 01_require.js's `initialize`, which now only
// fires on a require/node:module path). The global MessageChannel/MessagePort
// alias to the Node classes is observable as soon as node:worker_threads
// itself is imported (e.g. `import * as wt from "node:worker_threads"` and
// then `wt.MessagePort === MessagePort`), so install it here unconditionally
// when this module loads. The other half of __initWorkerThreads (parentPort,
// setupCrossThreadMessaging) still runs from initialize() / the worker eager
// bootstrap path because it depends on node:process being bootstrapped.
try {
  ObjectDefineProperty(globalThis, "MessageChannel", {
    __proto__: null,
    value: NodeMessageChannel,
    writable: true,
    enumerable: false,
    configurable: true,
  });
  ObjectDefineProperty(globalThis, "MessagePort", {
    __proto__: null,
    value: NodeMessagePort,
    writable: true,
    enumerable: false,
    configurable: true,
  });
} catch {
  // globalThis may be a sandbox without writable property descriptors; ignore.
}

return exportsObj;
})();
