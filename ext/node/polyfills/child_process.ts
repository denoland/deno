// Copyright 2018-2026 the Deno authors. MIT license.

// This module implements 'child_process' module of Node.JS API.
// ref: https://nodejs.org/api/child_process.html

(function () {
const { core, internals, primordials } = __bootstrap;
const {
  op_bootstrap_unstable_args,
  op_node_child_ipc_pipe,
  op_node_translate_cli_args,
} = core.ops;

const {
  ChildProcess,
  ChildProcessOptions,
  normalizeSpawnArguments,
  setupChannel,
  stdioStringToArray,
  validateNullByteNotInArg,
  default: internalChildProcess,
} = core.loadExtScript("ext:deno_node/internal/child_process.ts");
const {
  validateAbortSignal,
  validateFunction,
  validateInteger,
  validateNumber,
  validateObject,
  validateString,
} = core.loadExtScript("ext:deno_node/internal/validators.mjs");
const {
  ERR_CHILD_PROCESS_IPC_REQUIRED,
  ERR_CHILD_PROCESS_STDIO_MAXBUFFER,
  ERR_INVALID_ARG_TYPE,
  ERR_OUT_OF_RANGE,
  genericNodeError,
} = core.loadExtScript("ext:deno_node/internal/errors.ts");
const { getSystemErrorName, promisify } = core.loadExtScript(
  "ext:deno_node/util.ts",
);
const lazyProcess = core.createLazyLoader("node:process");
const process = lazyProcess().default;
const { Buffer } = core.loadExtScript("ext:deno_node/internal/buffer.mjs");
const {
  convertToValidSignal,
  kEmptyObject,
} = core.loadExtScript("ext:deno_node/internal/util.mjs");
const { toPathIfFileURL } = core.loadExtScript(
  "ext:deno_node/internal/url.ts",
);
const { kNeedsNpmProcessState } = core.loadExtScript(
  "ext:deno_process/40_process.js",
);

const {
  ArrayIsArray,
  ArrayPrototypeFilter,
  ArrayPrototypeIncludes,
  ArrayPrototypeJoin,
  ArrayPrototypeLastIndexOf,
  ArrayPrototypeMap,
  ArrayPrototypePush,
  ArrayPrototypeSlice,
  ArrayPrototypeSplice,
  Error,
  ObjectAssign,
  ObjectDefineProperty,
  Promise,
  PromiseWithResolvers,
  SafeArrayIterator,
  SafeSet,
  SetPrototypeHas,
  String,
  StringPrototypeSlice,
} = primordials;

const MAX_BUFFER = 1024 * 1024;

// Internal env var used to tell a compiled-binary child process (spawned by
// fork()) which embedded module to run as its main module instead of the
// baked-in entrypoint. Kept in sync with cli/rt/run.rs.
const INTERNAL_CHILD_ENTRYPOINT_ENV_VAR = "DENO_INTERNAL_CHILD_ENTRYPOINT";

// `Buffer.prototype.slice` is Buffer-specific (not the Array/String primordial),
// so the lint match on `.slice` and the spread are false positives here.
function bufferSlice(buf: string | Buffer, ...args: number[]) {
  // deno-lint-ignore prefer-primordials
  return buf.slice(...args);
}

type ForkOptions = ChildProcessOptions;

/**
 * Spawns a new Node.js process + fork.
 * @param modulePath
 * @param args
 * @param option
 * @returns
 */
function fork(
  modulePath: string | URL,
  _args?: string[],
  _options?: ForkOptions,
) {
  modulePath = toPathIfFileURL(modulePath) as string;
  validateString(modulePath, "modulePath");
  validateNullByteNotInArg(modulePath, "modulePath");

  // Get options and args arguments.
  let execArgv;
  let options: SpawnOptions & {
    execArgv?: string;
    execPath?: string;
    silent?: boolean;
  } = { __proto__: null } as typeof options;
  let args: string[] = [];
  let pos = 1;
  if (pos < arguments.length && ArrayIsArray(arguments[pos])) {
    args = arguments[pos++];
  }

  if (pos < arguments.length && arguments[pos] == null) {
    pos++;
  }

  if (pos < arguments.length && arguments[pos] != null) {
    if (typeof arguments[pos] !== "object" || ArrayIsArray(arguments[pos])) {
      throw new ERR_INVALID_ARG_TYPE(
        `arguments[${pos}]`,
        "object",
        arguments[pos],
      );
    }

    options = { __proto__: null, ...arguments[pos++] } as typeof options;
  }

  // Validate null bytes in args
  for (let i = 0; i < args.length; i++) {
    if (typeof args[i] === "string") {
      validateNullByteNotInArg(args[i], `args[${i}]`);
    }
  }

  // Validate null bytes in execPath
  if (options.execPath != null) {
    validateString(options.execPath, "options.execPath");
    validateNullByteNotInArg(options.execPath, "options.execPath");
  }

  // Validate null bytes in execArgv
  if (options.execArgv != null && ArrayIsArray(options.execArgv)) {
    for (let i = 0; i < options.execArgv.length; i++) {
      if (typeof options.execArgv[i] === "string") {
        validateNullByteNotInArg(
          options.execArgv[i],
          `options.execArgv[${i}]`,
        );
      }
    }
  }

  // Prepare arguments for fork:
  execArgv = options.execArgv || process.execArgv;

  if (execArgv === process.execArgv && process._eval != null) {
    const index = ArrayPrototypeLastIndexOf(execArgv, process._eval);
    if (index > 0) {
      // Remove the -e switch to avoid fork bombing ourselves.
      execArgv = ArrayPrototypeSlice(execArgv, 0);
      ArrayPrototypeSplice(execArgv, index - 1, 2);
    }
  }

  // Combine execArgv (Node CLI flags), modulePath (script), and args (script args)
  const nodeArgs = ArrayPrototypeMap(
    [
      ...new SafeArrayIterator(execArgv || []),
      modulePath,
      ...new SafeArrayIterator(args),
    ],
    String,
  );

  if (Deno.build.standalone) {
    // In standalone (compiled) binaries, skip Node-to-Deno arg translation.
    // The binary already has permissions and unstable config baked in.
    // Translating would inject "run -A --unstable-..." which the compiled
    // binary doesn't understand and would pass through as app args.
    args = nodeArgs;
    // A compiled binary always boots its baked-in entrypoint and ignores the
    // module path passed in argv, so without this a fork() would just re-run
    // the parent's entrypoint instead of `modulePath` (see issue #26304).
    // Tell the child which embedded module to run via an internal env var; the
    // standalone runtime resolves it against the entrypoint's directory inside
    // the compile VFS and runs it as the main module (see cli/rt/run.rs).
    options.env = {
      ...(options.env ?? process.env),
      [INTERNAL_CHILD_ENTRYPOINT_ENV_VAR]: modulePath,
    };
  } else {
    // Use the Rust parser to translate Node.js CLI args to Deno args
    // The parser handles Deno-style args (e.g., from vitest) by passing them through unchanged
    const result = op_node_translate_cli_args(nodeArgs, false, true);
    const denoArgs = result.denoArgs;
    const bootstrapArgs = op_bootstrap_unstable_args();

    // Insert bootstrap unstable args after "run" but before other args.
    // Filter out any that the translator already added to avoid duplicates
    // (e.g. --unstable-bare-node-builtins).
    // denoArgs is like ["run", "-A", "--unstable-...", "script.js", ...]
    // We need ["run", ...uniqueBootstrapArgs, "-A", "--unstable-...", "script.js", ...]
    const denoArgSet = new SafeSet(denoArgs);
    const uniqueBootstrapArgs = ArrayPrototypeFilter(
      bootstrapArgs,
      (a) => !SetPrototypeHas(denoArgSet, a),
    );
    if (
      denoArgs.length > 0 && denoArgs[0] === "run" &&
      uniqueBootstrapArgs.length > 0
    ) {
      args = [
        denoArgs[0],
        ...new SafeArrayIterator(uniqueBootstrapArgs),
        ...new SafeArrayIterator(ArrayPrototypeSlice(denoArgs, 1)),
      ];
    } else {
      args = [
        ...new SafeArrayIterator(uniqueBootstrapArgs),
        ...new SafeArrayIterator(denoArgs),
      ];
    }

    // Handle NODE_OPTIONS if the parser returned any
    if (result.nodeOptions.length > 0) {
      const nodeOptionsStr = ArrayPrototypeJoin(result.nodeOptions, " ");
      if (options.env) {
        options.env.NODE_OPTIONS = options.env.NODE_OPTIONS
          ? options.env.NODE_OPTIONS + " " + nodeOptionsStr
          : nodeOptionsStr;
      } else {
        options.env = { ...process.env, NODE_OPTIONS: nodeOptionsStr };
      }
    }
    if (result.caStores?.length) {
      options.env = {
        ...(options.env ?? process.env),
        DENO_TLS_CA_STORE: ArrayPrototypeJoin(result.caStores, ","),
      };
    }
    if (result.useOpensslCa) {
      options.env = {
        ...(options.env ?? process.env),
        DENO_NODE_USE_OPENSSL_CA: "1",
      };
    } else if (options.env?.DENO_NODE_USE_OPENSSL_CA) {
      delete options.env.DENO_NODE_USE_OPENSSL_CA;
    }
    if (result.traceEventCategories) {
      options.env = {
        ...(options.env ?? process.env),
        DENO_NODE_TRACE_EVENT_CATEGORIES: result.traceEventCategories,
      };
    }
  }

  if (typeof options.stdio === "string") {
    options.stdio = stdioStringToArray(options.stdio, "ipc");
  } else if (!ArrayIsArray(options.stdio)) {
    // Use a separate fd=3 for the IPC channel. Inherit stdin, stdout,
    // and stderr from the parent if silent isn't set.
    options.stdio = stdioStringToArray(
      options.silent ? "pipe" : "inherit",
      "ipc",
    );
  } else if (!ArrayPrototypeIncludes(options.stdio, "ipc")) {
    throw new ERR_CHILD_PROCESS_IPC_REQUIRED("options.stdio");
  }

  options.execPath = options.execPath || Deno.execPath();
  options.shell = false;

  // deno-lint-ignore no-explicit-any
  (options as any)[kNeedsNpmProcessState] = true;

  return spawn(options.execPath, args, options);
}

function spawn(
  command: string,
  argsOrOptions?: string[] | SpawnOptions,
  maybeOptions?: SpawnOptions,
): ChildProcess {
  const args = ArrayIsArray(argsOrOptions) ? argsOrOptions : [];
  let options = !ArrayIsArray(argsOrOptions) && argsOrOptions != null
    ? argsOrOptions
    : maybeOptions as SpawnOptions;

  options = normalizeSpawnArguments(command, args, options);

  validateAbortSignal(options?.signal, "options.signal");
  validateTimeout(options?.timeout);

  const child = new ChildProcess();
  child.spawn(options);

  const timeout = options?.timeout;
  if (timeout != null && timeout > 0) {
    const killSignal = options?.killSignal ?? "SIGTERM";
    let timeoutId: ReturnType<typeof setTimeout> | null = setTimeout(() => {
      timeoutId = null;
      child.kill(killSignal as string);
    }, timeout);

    child.once("exit", () => {
      if (timeoutId) {
        clearTimeout(timeoutId);
        timeoutId = null;
      }
    });
  }

  return child;
}

function validateTimeout(timeout?: number) {
  if (timeout != null) {
    validateInteger(timeout, "timeout", 0);
  }
}

function validateMaxBuffer(maxBuffer?: number) {
  if (maxBuffer != null) {
    validateNumber(maxBuffer, "options.maxBuffer", 0);
  }
}

function sanitizeKillSignal(killSignal?: string | number) {
  if (typeof killSignal === "string" || typeof killSignal === "number") {
    return convertToValidSignal(killSignal);
  } else if (killSignal != null) {
    throw new ERR_INVALID_ARG_TYPE(
      "options.killSignal",
      ["string", "number"],
      killSignal,
    );
  }
}

function spawnSync(
  command: string,
  argsOrOptions?: string[] | SpawnSyncOptions,
  maybeOptions?: SpawnSyncOptions,
): SpawnSyncResult {
  const args = ArrayIsArray(argsOrOptions) ? argsOrOptions : [];
  let options = !ArrayIsArray(argsOrOptions) && argsOrOptions
    ? argsOrOptions
    : maybeOptions as SpawnSyncOptions;

  options = {
    __proto__: null,
    maxBuffer: MAX_BUFFER,
    ...normalizeSpawnArguments(command, args, options),
  } as typeof options;

  // Validate the timeout, if present.
  validateTimeout(options.timeout);

  // Validate maxBuffer, if present.
  validateMaxBuffer(options.maxBuffer);

  // Validate and translate the kill signal, if present.
  options.killSignal = sanitizeKillSignal(options.killSignal);

  return internalChildProcess.spawnSync(options);
}

interface ExecOptions extends
  Pick<
    ChildProcessOptions,
    | "env"
    | "signal"
    | "uid"
    | "gid"
    | "windowsHide"
  > {
  cwd?: string | URL;
  encoding?: string;
  /**
   * Shell to execute the command with.
   */
  shell?: string;
  timeout?: number;
  /**
   * Largest amount of data in bytes allowed on stdout or stderr. If exceeded, the child process is terminated and any output is truncated.
   */
  maxBuffer?: number;
  killSignal?: string | number;
}
type ExecException = ChildProcessError;
type ExecCallback = (
  error: ExecException | null,
  stdout?: string | Buffer,
  stderr?: string | Buffer,
) => void;
type ExecSyncOptions = SpawnSyncOptions;
type ExecFileSyncOptions = SpawnSyncOptions;
function normalizeExecArgs(
  command: string,
  optionsOrCallback?: ExecOptions | ExecSyncOptions | ExecCallback,
  maybeCallback?: ExecCallback,
) {
  let callback: ExecFileCallback | undefined = maybeCallback;

  if (typeof optionsOrCallback === "function") {
    callback = optionsOrCallback;
    optionsOrCallback = undefined;
  }

  // Make a shallow copy so we don't clobber the user's options object.
  const options: ExecOptions | ExecSyncOptions = {
    __proto__: null,
    ...optionsOrCallback,
  } as ExecOptions | ExecSyncOptions;
  options.shell = typeof options.shell === "string" ? options.shell : true;

  return {
    file: command,
    options: options!,
    callback: callback!,
  };
}

/**
 * Spawns a shell executing the given command.
 */
function exec(
  command: string,
  optionsOrCallback?: ExecOptions | ExecCallback,
  maybeCallback?: ExecCallback,
): ChildProcess {
  const opts = normalizeExecArgs(command, optionsOrCallback, maybeCallback);
  return execFile(opts.file, opts.options as ExecFileOptions, opts.callback);
}

interface PromiseWithChild<T> extends Promise<T> {
  child: ChildProcess;
}
type ExecOutputForPromisify = {
  stdout?: string | Buffer;
  stderr?: string | Buffer;
};
type ExecExceptionForPromisify = ExecException & ExecOutputForPromisify;

const customPromiseExecFunction = (orig: typeof exec) => {
  // Give the returned function the same name as the original so
  // `promisify(exec).name === 'exec'`, matching Node's
  // `assignFunctionName(orig.name, ...)` (see lib/child_process.js).
  const fn = (...args: [command: string, options: ExecOptions]) => {
    const { promise, resolve, reject } = PromiseWithResolvers();

    promise.child = orig(
      ...new SafeArrayIterator(args),
      (err, stdout, stderr) => {
        if (err !== null) {
          const _err: ExecExceptionForPromisify = err;
          _err.stdout = stdout;
          _err.stderr = stderr;
          reject && reject(_err);
        } else {
          resolve && resolve({ stdout, stderr });
        }
      },
    );

    return promise;
  };
  ObjectDefineProperty(fn, "name", {
    __proto__: null,
    value: orig.name,
    configurable: true,
  });
  return fn;
};

ObjectDefineProperty(exec, promisify.custom, {
  __proto__: null,
  enumerable: false,
  value: customPromiseExecFunction(exec),
});

interface ExecFileOptions extends ChildProcessOptions {
  encoding?: string;
  timeout?: number;
  maxBuffer?: number;
  killSignal?: string | number;
}
interface ChildProcessError extends Error {
  code?: string | number;
  killed?: boolean;
  signal?: AbortSignal;
  cmd?: string;
}
class ExecFileError extends Error implements ChildProcessError {
  code?: string | number;

  constructor(message: string) {
    super(message);
    this.code = "UNKNOWN";
  }
}
type ExecFileCallback = (
  error: ChildProcessError | null,
  stdout?: string | Buffer,
  stderr?: string | Buffer,
) => void;
function execFile(
  file: string,
  argsOrOptionsOrCallback?: string[] | ExecFileOptions | ExecFileCallback,
  optionsOrCallback?: ExecFileOptions | ExecFileCallback,
  maybeCallback?: ExecFileCallback,
): ChildProcess {
  let args: string[] = [];
  let options: ExecFileOptions = {};
  let callback: ExecFileCallback | undefined;

  if (ArrayIsArray(argsOrOptionsOrCallback)) {
    args = argsOrOptionsOrCallback;
  } else if (typeof argsOrOptionsOrCallback === "function") {
    callback = argsOrOptionsOrCallback;
    // When second arg is callback, ignore remaining args
  } else if (argsOrOptionsOrCallback != null) {
    if (typeof argsOrOptionsOrCallback !== "object") {
      throw new ERR_INVALID_ARG_TYPE(
        "args",
        ["object", "array"],
        argsOrOptionsOrCallback,
      );
    }
    options = argsOrOptionsOrCallback;
  }
  // Only process subsequent args if callback wasn't set from second arg
  if (callback === undefined) {
    if (typeof optionsOrCallback === "function") {
      callback = optionsOrCallback;
    } else if (optionsOrCallback != null) {
      if (
        typeof optionsOrCallback !== "object" ||
        ArrayIsArray(optionsOrCallback)
      ) {
        throw new ERR_INVALID_ARG_TYPE(
          "options",
          "object",
          optionsOrCallback,
        );
      }
      options = optionsOrCallback;
      callback = maybeCallback;
    }
    // Validate callback if provided
    if (maybeCallback != null && typeof maybeCallback !== "function") {
      throw new ERR_INVALID_ARG_TYPE("callback", "function", maybeCallback);
    }
  }

  const execOptions = {
    __proto__: null,
    encoding: "utf8",
    timeout: 0,
    maxBuffer: MAX_BUFFER,
    killSignal: "SIGTERM",
    shell: false,
    ...options,
  };
  validateTimeout(execOptions.timeout);
  if (execOptions.maxBuffer < 0) {
    throw new ERR_OUT_OF_RANGE(
      "options.maxBuffer",
      "a positive number",
      execOptions.maxBuffer,
    );
  }
  const spawnOptions: SpawnOptions = {
    argv0: execOptions.argv0,
    cwd: execOptions.cwd,
    env: execOptions.env,
    gid: execOptions.gid,
    shell: execOptions.shell,
    signal: execOptions.signal,
    uid: execOptions.uid,
    windowsHide: execOptions.windowsHide !== false,
    windowsVerbatimArguments: !!execOptions.windowsVerbatimArguments,
  };

  const child = spawn(file, args, spawnOptions);

  let encoding: string | null;
  const _stdout: (string | Uint8Array)[] = [];
  const _stderr: (string | Uint8Array)[] = [];
  if (
    execOptions.encoding !== "buffer" &&
    Buffer.isEncoding(execOptions.encoding)
  ) {
    encoding = execOptions.encoding;
  } else {
    encoding = null;
  }
  let stdoutLen = 0;
  let stderrLen = 0;
  let killed = false;
  let exited = false;
  let timeoutId: number | null;

  let ex: ChildProcessError | null = null;

  let cmd = file;

  function exithandler(code = 0, signal?: AbortSignal) {
    if (exited) return;
    exited = true;

    if (timeoutId) {
      clearTimeout(timeoutId);
      timeoutId = null;
    }

    if (!callback) return;

    // merge chunks
    let stdout;
    let stderr;
    if (
      encoding ||
      (
        child.stdout &&
        child.stdout.readableEncoding
      )
    ) {
      stdout = ArrayPrototypeJoin(_stdout, "");
    } else {
      // deno-lint-ignore prefer-primordials
      stdout = Buffer.concat(_stdout as Buffer[]);
    }
    if (
      encoding ||
      (
        child.stderr &&
        child.stderr.readableEncoding
      )
    ) {
      stderr = ArrayPrototypeJoin(_stderr, "");
    } else {
      // deno-lint-ignore prefer-primordials
      stderr = Buffer.concat(_stderr as Buffer[]);
    }

    if (!ex && code === 0 && signal === null) {
      callback(null, stdout, stderr);
      return;
    }

    if (args?.length) {
      cmd += ` ${ArrayPrototypeJoin(args, " ")}`;
    }

    if (!ex) {
      ex = new ExecFileError(
        "Command failed: " + cmd + "\n" + stderr,
      );
      ex.code = code < 0 ? getSystemErrorName(code) : code;
      ex.killed = child.killed || killed;
      ex.signal = signal;
    }

    ex.cmd = cmd;
    callback(ex, stdout, stderr);
  }

  function errorhandler(e: ExecFileError) {
    ex = e;

    if (child.stdout) {
      child.stdout.destroy();
    }

    if (child.stderr) {
      child.stderr.destroy();
    }

    exithandler();
  }

  function kill() {
    if (child.stdout) {
      child.stdout.destroy();
    }

    if (child.stderr) {
      child.stderr.destroy();
    }

    killed = true;
    try {
      child.kill(execOptions.killSignal);
    } catch (e) {
      if (e) {
        ex = e as ChildProcessError;
      }
      exithandler();
    }
  }

  if (execOptions.timeout > 0) {
    timeoutId = setTimeout(function delayedKill() {
      kill();
      timeoutId = null;
    }, execOptions.timeout);
  }

  if (child.stdout) {
    if (encoding) {
      child.stdout.setEncoding(encoding);
    }

    child.stdout.on("data", function onChildStdout(chunk: string | Buffer) {
      // Do not need to count the length
      if (execOptions.maxBuffer === Infinity) {
        ArrayPrototypePush(_stdout, chunk);
        return;
      }

      const encoding = child.stdout?.readableEncoding;
      const length = encoding
        ? Buffer.byteLength(chunk, encoding)
        : chunk.length;
      const slice = encoding ? StringPrototypeSlice : bufferSlice;
      stdoutLen += length;

      if (stdoutLen > execOptions.maxBuffer) {
        const truncatedLen = execOptions.maxBuffer - (stdoutLen - length);
        ArrayPrototypePush(_stdout, slice(chunk, 0, truncatedLen));

        ex = new ERR_CHILD_PROCESS_STDIO_MAXBUFFER("stdout");
        kill();
      } else {
        ArrayPrototypePush(_stdout, chunk);
      }
    });
  }

  if (child.stderr) {
    if (encoding) {
      child.stderr.setEncoding(encoding);
    }

    child.stderr.on("data", function onChildStderr(chunk: string | Buffer) {
      // Do not need to count the length
      if (execOptions.maxBuffer === Infinity) {
        ArrayPrototypePush(_stderr, chunk);
        return;
      }

      const encoding = child.stderr?.readableEncoding;
      const length = encoding
        ? Buffer.byteLength(chunk, encoding)
        : chunk.length;
      const slice = encoding ? StringPrototypeSlice : bufferSlice;
      stderrLen += length;

      if (stderrLen > execOptions.maxBuffer) {
        const truncatedLen = execOptions.maxBuffer - (stderrLen - length);
        ArrayPrototypePush(_stderr, slice(chunk, 0, truncatedLen));

        ex = new ERR_CHILD_PROCESS_STDIO_MAXBUFFER("stderr");
        kill();
      } else {
        ArrayPrototypePush(_stderr, chunk);
      }
    });
  }

  child.addListener("close", exithandler);
  child.addListener("error", errorhandler);

  return child;
}

type ExecFileExceptionForPromisify = ExecFileError & ExecOutputForPromisify;

const customPromiseExecFileFunction = (
  orig: (
    file: string,
    argsOrOptionsOrCallback?: string[] | ExecFileOptions | ExecFileCallback,
    optionsOrCallback?: ExecFileOptions | ExecFileCallback,
    maybeCallback?: ExecFileCallback,
  ) => ChildProcess,
) => {
  const fn = (
    ...args: [
      file: string,
      argsOrOptions?: string[] | ExecFileOptions,
      options?: ExecFileOptions,
    ]
  ) => {
    const { promise, resolve, reject } = PromiseWithResolvers();

    promise.child = orig(
      ...new SafeArrayIterator(args),
      (err, stdout, stderr) => {
        if (err !== null) {
          const _err: ExecFileExceptionForPromisify = err;
          _err.stdout = stdout;
          _err.stderr = stderr;
          reject && reject(_err);
        } else {
          resolve && resolve({ stdout, stderr });
        }
      },
    );

    return promise;
  };
  ObjectDefineProperty(fn, "name", {
    __proto__: null,
    value: orig.name,
    configurable: true,
  });
  return fn;
};

ObjectDefineProperty(execFile, promisify.custom, {
  __proto__: null,
  enumerable: false,
  value: customPromiseExecFileFunction(execFile),
});

function checkExecSyncError(
  ret: SpawnSyncResult,
  args: string[],
  cmd?: string,
) {
  let err;
  if (ret.error) {
    err = ret.error;
    ObjectAssign(err, ret);
  } else if (ret.status !== 0) {
    let msg = "Command failed: ";
    msg += cmd || ArrayPrototypeJoin(args, " ");
    if (ret.stderr && ret.stderr.length > 0) {
      // deno-lint-ignore prefer-primordials
      msg += `\n${ret.stderr.toString()}`;
    }
    err = genericNodeError(msg, ret);
  }
  return err;
}

function execSync(command: string, options: ExecSyncOptions) {
  const opts = normalizeExecArgs(command, options);
  const inheritStderr = !(opts.options as ExecSyncOptions).stdio;

  const ret = spawnSync(opts.file, opts.options as SpawnSyncOptions);

  if (inheritStderr && ret.stderr) {
    process.stderr.write(ret.stderr);
  }

  const err = checkExecSyncError(ret, [], command);

  if (err) {
    throw err;
  }

  return ret.stdout;
}

function normalizeExecFileArgs(
  file: string,
  args?: string[] | null | ExecFileSyncOptions | ExecFileCallback,
  options?: ExecFileSyncOptions | null | ExecFileCallback,
  callback?: ExecFileCallback,
): {
  file: string;
  args: string[];
  options: ExecFileSyncOptions;
  callback?: ExecFileCallback;
} {
  if (ArrayIsArray(args)) {
    args = ArrayPrototypeSlice(args);
  } else if (args != null && typeof args === "object") {
    callback = options as ExecFileCallback;
    options = args as ExecFileSyncOptions;
    args = null;
  } else if (typeof args === "function") {
    callback = args;
    options = null;
    args = null;
  }

  if (args == null) {
    args = [];
  }

  if (typeof options === "function") {
    callback = options as ExecFileCallback;
  } else if (options != null) {
    validateObject(options, "options");
  }

  if (options == null) {
    options = kEmptyObject;
  }

  args = args as string[];
  options = options as ExecFileSyncOptions;

  if (callback != null) {
    validateFunction(callback, "callback");
  }

  // Validate argv0, if present.
  if (options.argv0 != null) {
    validateString(options.argv0, "options.argv0");
  }

  return { file, args, options, callback };
}

function execFileSync(
  file: string,
  args?: string[] | ExecFileSyncOptions,
  options?: ExecFileSyncOptions,
): string | Buffer {
  ({ file, args, options } = normalizeExecFileArgs(file, args, options));

  const inheritStderr = !options.stdio;
  const ret = spawnSync(file, args, options);

  if (inheritStderr && ret.stderr) {
    process.stderr.write(ret.stderr);
  }

  const errArgs: string[] = [
    options.argv0 || file,
    ...new SafeArrayIterator(args as string[]),
  ];
  const err = checkExecSyncError(ret, errArgs);

  if (err) {
    throw err;
  }

  return ret.stdout as string | Buffer;
}

function setupChildProcessIpcChannel() {
  const maybePipe = op_node_child_ipc_pipe();
  if (!maybePipe) return;
  const fd = maybePipe[0];
  const serialization = maybePipe[1];
  const serializationMode = serialization === 0 ? "json" : "advanced";
  if (typeof fd != "number" || fd < 0) return;
  const control = setupChannel(process, fd, serializationMode);
  process.on("newListener", (name: string) => {
    if (name === "message" || name === "disconnect") {
      control.refCounted();
    }
  });
  process.on("removeListener", (name: string) => {
    if (name === "message" || name === "disconnect") {
      control.unrefCounted();
    }
  });
}

internals.__setupChildProcessIpcChannel = setupChildProcessIpcChannel;

return {
  fork,
  spawn,
  exec,
  execFile,
  execFileSync,
  execSync,
  ChildProcess,
  spawnSync,
};
})();
