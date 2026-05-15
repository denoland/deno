// Copyright 2018-2026 the Deno authors. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials ban-types no-this-alias

(function () {
const { core, primordials } = globalThis.__bootstrap;
const { WasiContext } = core.ops;
const { statSync } = core.loadExtScript("ext:deno_node/fs.ts");
const { exit } = core.loadExtScript("ext:deno_os/30_os.js");
const {
  ERR_INVALID_ARG_TYPE,
  ERR_INVALID_ARG_VALUE,
  ERR_WASI_ALREADY_STARTED,
  ERR_WASI_NOT_STARTED,
} = core.loadExtScript("ext:deno_node/internal/errors.ts");

const {
  ArrayPrototypeMap,
  ArrayIsArray,
  Error,
  NumberIsInteger,
  ObjectEntries,
  ObjectPrototypeToString,
  String,
  TypeError,
  Uint8Array,
} = primordials;

// UVWASI error for path not found
class UVWASIError extends Error {
  code: string;
  constructor(code: string, message: string) {
    super(message);
    this.code = code;
    this.name = "Error";
  }
}

// Check if value is a WebAssembly.Memory, works across VM contexts
function isWasmMemory(value: unknown): boolean {
  // instanceof fails across VM contexts, use Object.prototype.toString
  return ObjectPrototypeToString(value) === "[object WebAssembly.Memory]";
}

// Custom TypeError with ERR_INVALID_ARG_TYPE code for memory validation.
// Node uses native THROW_ERR_INVALID_ARG_TYPE which bypasses the JS formatter.
function createMemoryTypeError(actual: unknown): TypeError {
  const received = actual === undefined
    ? "undefined"
    : actual === null
    ? "null"
    : `type ${typeof actual}`;
  const err = new TypeError(
    `The "instance.exports.memory" property must be a WebAssembly.Memory object. Received ${received}`,
  );
  (err as unknown as { code: string }).code = "ERR_INVALID_ARG_TYPE";
  return err;
}

class WASIProcExit {
  code: number;
  constructor(code: number) {
    this.code = code;
  }
}

function validateObject(
  value: unknown,
  name: string,
): asserts value is object {
  if (value === null || typeof value !== "object" || ArrayIsArray(value)) {
    throw new ERR_INVALID_ARG_TYPE(name, "object", value);
  }
}

function validateArray(
  value: unknown,
  name: string,
): asserts value is unknown[] {
  if (!ArrayIsArray(value)) {
    throw new ERR_INVALID_ARG_TYPE(name, "Array", value);
  }
}

function validateBoolean(
  value: unknown,
  name: string,
): asserts value is boolean {
  if (typeof value !== "boolean") {
    throw new ERR_INVALID_ARG_TYPE(name, "boolean", value);
  }
}

function validateInt32(value: unknown, name: string): asserts value is number {
  if (!NumberIsInteger(value)) {
    throw new ERR_INVALID_ARG_TYPE(name, "int32", value);
  }
}

function validateString(
  value: unknown,
  name: string,
): asserts value is string {
  if (typeof value !== "string") {
    throw new ERR_INVALID_ARG_TYPE(name, "string", value);
  }
}

// deno-lint-ignore no-explicit-any
type WasiOptions = any;

// WASI preopens use direct host filesystem access in the native ops. They do
// not read from the in-memory VFS that `deno compile` uses for embedded files.
class WASI {
  #ctx;
  #version: string;
  #started = false;
  #returnOnExit: boolean;
  #wasiImport;

  constructor(options?: WasiOptions) {
    if (options === undefined) {
      throw new ERR_INVALID_ARG_TYPE("options.version", "string", undefined);
    }
    validateObject(options, "options");

    if (options.version === undefined) {
      throw new ERR_INVALID_ARG_TYPE("options.version", "string", undefined);
    }
    validateString(options.version, "options.version");
    if (options.version !== "preview1" && options.version !== "unstable") {
      throw new ERR_INVALID_ARG_VALUE(
        "options.version",
        options.version,
        'must be "preview1" or "unstable"',
      );
    }

    const argsValue = options.args ?? [];
    if (options.args !== undefined) {
      validateArray(options.args, "options.args");
    }
    const args = ArrayPrototypeMap(argsValue, (arg) => String(arg));

    const envObj = options.env ?? {};
    if (options.env !== undefined) {
      validateObject(options.env, "options.env");
    }
    const envPairs: [string, string][] = [];
    for (const [key, value] of ObjectEntries(envObj)) {
      envPairs.push([key, String(value)]);
    }

    if (options.preopens !== undefined) {
      validateObject(options.preopens, "options.preopens");
    }
    const preopens: [string, string][] = [];
    if (options.preopens) {
      for (const [virtualPath, realPath] of ObjectEntries(options.preopens)) {
        const realPathString = String(realPath);
        try {
          statSync(realPathString);
        } catch {
          throw new UVWASIError(
            "UVWASI_ENOENT",
            `uvwasi_init: failed to open preopen "${realPathString}"`,
          );
        }
        preopens.push([String(virtualPath), realPathString]);
      }
    }

    if (options.returnOnExit !== undefined) {
      validateBoolean(options.returnOnExit, "options.returnOnExit");
    }

    if (options.stdin !== undefined) {
      validateInt32(options.stdin, "options.stdin");
    }
    if (options.stdout !== undefined) {
      validateInt32(options.stdout, "options.stdout");
    }
    if (options.stderr !== undefined) {
      validateInt32(options.stderr, "options.stderr");
    }

    const stdinFd = options.stdin ?? 0;
    const stdoutFd = options.stdout ?? 1;
    const stderrFd = options.stderr ?? 2;
    this.#returnOnExit = options.returnOnExit ?? true;
    this.#version = options.version;

    this.#ctx = new WasiContext(
      args,
      envPairs,
      preopens,
      stdinFd,
      stdoutFd,
      stderrFd,
      this.#returnOnExit,
    );

    // Build the wasiImport object. Each function delegates to the cppgc object
    // method, passing the wasm memory buffer.
    const ctx = this.#ctx;
    const self = this;

    this.#wasiImport = {
      args_get(argv: number, argvBuf: number) {
        return ctx.argsGet(argv, argvBuf, self.#getMemoryBuffer());
      },
      args_sizes_get(argc: number, argvBufSize: number) {
        return ctx.argsSizesGet(argc, argvBufSize, self.#getMemoryBuffer());
      },
      environ_get(environ: number, environBuf: number) {
        return ctx.environGet(environ, environBuf, self.#getMemoryBuffer());
      },
      environ_sizes_get(environCount: number, environBufSize: number) {
        return ctx.environSizesGet(
          environCount,
          environBufSize,
          self.#getMemoryBuffer(),
        );
      },
      clock_res_get(clockId: number, resolution: number) {
        return ctx.clockResGet(
          clockId,
          resolution,
          self.#getMemoryBuffer(),
        );
      },
      clock_time_get(clockId: number, precision: number, time: number) {
        return ctx.clockTimeGet(
          clockId,
          precision,
          time,
          self.#getMemoryBuffer(),
        );
      },
      random_get(bufPtr: number, bufLen: number) {
        return ctx.randomGet(bufPtr, bufLen, self.#getMemoryBuffer());
      },
      proc_exit(code: number) {
        const exitCode = ctx.procExit(code);
        if (self.#returnOnExit) {
          throw new WASIProcExit(exitCode);
        }
        exit(exitCode);
      },
      proc_raise(sig: number) {
        return ctx.procRaise(sig);
      },
      fd_write(
        fd: number,
        iovsPtr: number,
        iovsLen: number,
        nwrittenPtr: number,
      ) {
        return ctx.fdWrite(
          fd,
          iovsPtr,
          iovsLen,
          nwrittenPtr,
          self.#getMemoryBuffer(),
        );
      },
      fd_read(
        fd: number,
        iovsPtr: number,
        iovsLen: number,
        nreadPtr: number,
      ) {
        return ctx.fdRead(
          fd,
          iovsPtr,
          iovsLen,
          nreadPtr,
          self.#getMemoryBuffer(),
        );
      },
      fd_seek(
        fd: number,
        offset: number,
        whence: number,
        newoffsetPtr: number,
      ) {
        return ctx.fdSeek(
          fd,
          offset,
          whence,
          newoffsetPtr,
          self.#getMemoryBuffer(),
        );
      },
      fd_close(fd: number) {
        return ctx.fdClose(fd);
      },
      fd_fdstat_get(fd: number, fdstatPtr: number) {
        return ctx.fdFdstatGet(fd, fdstatPtr, self.#getMemoryBuffer());
      },
      fd_fdstat_set_flags(fd: number, flags: number) {
        return ctx.fdFdstatSetFlags(fd, flags);
      },
      fd_fdstat_set_rights(
        fd: number,
        fsRightsBase: number,
        fsRightsInheriting: number,
      ) {
        return ctx.fdFdstatSetRights(
          fd,
          fsRightsBase,
          fsRightsInheriting,
        );
      },
      fd_prestat_get(fd: number, prestatPtr: number) {
        return ctx.fdPrestatGet(fd, prestatPtr, self.#getMemoryBuffer());
      },
      fd_prestat_dir_name(fd: number, pathPtr: number, pathLen: number) {
        return ctx.fdPrestatDirName(
          fd,
          pathPtr,
          pathLen,
          self.#getMemoryBuffer(),
        );
      },
      fd_tell(fd: number, offsetPtr: number) {
        return ctx.fdTell(fd, offsetPtr, self.#getMemoryBuffer());
      },
      fd_sync(fd: number) {
        return ctx.fdSync(fd);
      },
      fd_datasync(fd: number) {
        return ctx.fdDatasync(fd);
      },
      fd_advise(fd: number, offset: number, len: number, advice: number) {
        return ctx.fdAdvise(fd, offset, len, advice);
      },
      fd_allocate(fd: number, offset: number, len: number) {
        return ctx.fdAllocate(fd, offset, len);
      },
      fd_filestat_get(fd: number, filestatPtr: number) {
        return ctx.fdFilestatGet(
          fd,
          filestatPtr,
          self.#getMemoryBuffer(),
        );
      },
      fd_filestat_set_size(fd: number, size: number) {
        return ctx.fdFilestatSetSize(fd, size);
      },
      fd_filestat_set_times(
        fd: number,
        atim: number,
        mtim: number,
        fstFlags: number,
      ) {
        return ctx.fdFilestatSetTimes(fd, atim, mtim, fstFlags);
      },
      fd_renumber(from: number, to: number) {
        return ctx.fdRenumber(from, to);
      },
      fd_readdir(
        fd: number,
        bufPtr: number,
        bufLen: number,
        cookie: number,
        bufusedPtr: number,
      ) {
        return ctx.fdReaddir(
          fd,
          bufPtr,
          bufLen,
          cookie,
          bufusedPtr,
          self.#getMemoryBuffer(),
        );
      },
      fd_pread(
        fd: number,
        iovsPtr: number,
        iovsLen: number,
        offset: number,
        nreadPtr: number,
      ) {
        return ctx.fdPread(
          fd,
          iovsPtr,
          iovsLen,
          offset,
          nreadPtr,
          self.#getMemoryBuffer(),
        );
      },
      fd_pwrite(
        fd: number,
        iovsPtr: number,
        iovsLen: number,
        offset: number,
        nwrittenPtr: number,
      ) {
        return ctx.fdPwrite(
          fd,
          iovsPtr,
          iovsLen,
          offset,
          nwrittenPtr,
          self.#getMemoryBuffer(),
        );
      },
      path_open(
        dirfd: number,
        dirflags: number,
        pathPtr: number,
        pathLen: number,
        oflags: number,
        fsRightsBase: number,
        fsRightsInheriting: number,
        fdflags: number,
        fdPtr: number,
      ) {
        return ctx.pathOpen(
          dirfd,
          dirflags,
          pathPtr,
          pathLen,
          oflags,
          fsRightsBase,
          fsRightsInheriting,
          fdflags,
          fdPtr,
          self.#getMemoryBuffer(),
        );
      },
      path_create_directory(
        dirfd: number,
        pathPtr: number,
        pathLen: number,
      ) {
        return ctx.pathCreateDirectory(
          dirfd,
          pathPtr,
          pathLen,
          self.#getMemoryBuffer(),
        );
      },
      path_remove_directory(
        dirfd: number,
        pathPtr: number,
        pathLen: number,
      ) {
        return ctx.pathRemoveDirectory(
          dirfd,
          pathPtr,
          pathLen,
          self.#getMemoryBuffer(),
        );
      },
      path_unlink_file(dirfd: number, pathPtr: number, pathLen: number) {
        return ctx.pathUnlinkFile(
          dirfd,
          pathPtr,
          pathLen,
          self.#getMemoryBuffer(),
        );
      },
      path_rename(
        oldDirfd: number,
        oldPathPtr: number,
        oldPathLen: number,
        newDirfd: number,
        newPathPtr: number,
        newPathLen: number,
      ) {
        return ctx.pathRename(
          oldDirfd,
          oldPathPtr,
          oldPathLen,
          newDirfd,
          newPathPtr,
          newPathLen,
          self.#getMemoryBuffer(),
        );
      },
      path_filestat_get(
        dirfd: number,
        flags: number,
        pathPtr: number,
        pathLen: number,
        filestatPtr: number,
      ) {
        return ctx.pathFilestatGet(
          dirfd,
          flags,
          pathPtr,
          pathLen,
          filestatPtr,
          self.#getMemoryBuffer(),
        );
      },
      path_readlink(
        dirfd: number,
        pathPtr: number,
        pathLen: number,
        bufPtr: number,
        bufLen: number,
        bufusedPtr: number,
      ) {
        return ctx.pathReadlink(
          dirfd,
          pathPtr,
          pathLen,
          bufPtr,
          bufLen,
          bufusedPtr,
          self.#getMemoryBuffer(),
        );
      },
      path_symlink(
        oldPathPtr: number,
        oldPathLen: number,
        dirfd: number,
        newPathPtr: number,
        newPathLen: number,
      ) {
        return ctx.pathSymlink(
          oldPathPtr,
          oldPathLen,
          dirfd,
          newPathPtr,
          newPathLen,
          self.#getMemoryBuffer(),
        );
      },
      path_filestat_set_times(
        dirfd: number,
        flags: number,
        pathPtr: number,
        pathLen: number,
        atim: number,
        mtim: number,
        fstFlags: number,
      ) {
        return ctx.pathFilestatSetTimes(
          dirfd,
          flags,
          pathPtr,
          pathLen,
          atim,
          mtim,
          fstFlags,
          self.#getMemoryBuffer(),
        );
      },
      poll_oneoff(
        inPtr: number,
        outPtr: number,
        nsubscriptions: number,
        neventsPtr: number,
      ) {
        return ctx.pollOneoff(
          inPtr,
          outPtr,
          nsubscriptions,
          neventsPtr,
          self.#getMemoryBuffer(),
        );
      },
      sched_yield() {
        return ctx.schedYield();
      },
      sock_recv(
        fd: number,
        riDataPtr: number,
        riDataLen: number,
        riFlags: number,
        roDatalenPtr: number,
        roFlagsPtr: number,
      ) {
        return ctx.sockRecv(
          fd,
          riDataPtr,
          riDataLen,
          riFlags,
          roDatalenPtr,
          roFlagsPtr,
          self.#getMemoryBuffer(),
        );
      },
      sock_send(
        fd: number,
        siDataPtr: number,
        siDataLen: number,
        siFlags: number,
        soDatalenPtr: number,
      ) {
        return ctx.sockSend(
          fd,
          siDataPtr,
          siDataLen,
          siFlags,
          soDatalenPtr,
          self.#getMemoryBuffer(),
        );
      },
      sock_shutdown(fd: number, how: number) {
        return ctx.sockShutdown(fd, how);
      },
      sock_accept(fd: number, flags: number, fdPtr: number) {
        return ctx.sockAccept(fd, flags, fdPtr, self.#getMemoryBuffer());
      },
    };
  }

  #memory: WebAssembly.Memory | null = null;

  #getMemoryBuffer(): Uint8Array {
    if (!this.#memory) {
      throw new ERR_WASI_NOT_STARTED();
    }
    return new Uint8Array(this.#memory.buffer);
  }

  get wasiImport() {
    return this.#wasiImport;
  }

  getImportObject() {
    if (this.#version === "unstable") {
      return { wasi_unstable: this.#wasiImport };
    }
    return { wasi_snapshot_preview1: this.#wasiImport };
  }

  start(instance?: WebAssembly.Instance): number {
    if (this.#started) {
      throw new ERR_WASI_ALREADY_STARTED();
    }

    if (instance === undefined || instance === null) {
      throw new ERR_INVALID_ARG_TYPE("instance", "object", instance);
    }
    if (typeof instance !== "object") {
      throw new ERR_INVALID_ARG_TYPE("instance", "object", instance);
    }

    const exports = instance.exports;
    if (exports === null || typeof exports !== "object") {
      throw new ERR_INVALID_ARG_TYPE("instance.exports", "object", exports);
    }

    if (typeof exports._start !== "function") {
      throw new ERR_INVALID_ARG_TYPE(
        "instance.exports._start",
        "function",
        exports._start,
      );
    }

    if (exports._initialize !== undefined) {
      throw new ERR_INVALID_ARG_TYPE(
        "instance.exports._initialize",
        "undefined",
        exports._initialize,
      );
    }

    if (!isWasmMemory(exports.memory)) {
      throw createMemoryTypeError(exports.memory);
    }

    this.#memory = exports.memory as WebAssembly.Memory;
    this.#started = true;

    try {
      (exports._start as Function)();
    } catch (e) {
      if (e instanceof WASIProcExit) {
        return e.code;
      }
      throw e;
    }

    return 0;
  }

  initialize(instance?: WebAssembly.Instance): void {
    if (this.#started) {
      throw new ERR_WASI_ALREADY_STARTED();
    }

    if (instance === undefined || instance === null) {
      throw new ERR_INVALID_ARG_TYPE("instance", "object", instance);
    }
    if (typeof instance !== "object") {
      throw new ERR_INVALID_ARG_TYPE("instance", "object", instance);
    }

    const exports = instance.exports;
    if (exports === null || typeof exports !== "object") {
      throw new ERR_INVALID_ARG_TYPE("instance.exports", "object", exports);
    }

    if (
      exports._initialize !== undefined &&
      typeof exports._initialize !== "function"
    ) {
      throw new ERR_INVALID_ARG_TYPE(
        "instance.exports._initialize",
        "function",
        exports._initialize,
      );
    }

    if (exports._start !== undefined) {
      throw new ERR_INVALID_ARG_TYPE(
        "instance.exports._start",
        "undefined",
        exports._start,
      );
    }

    if (!isWasmMemory(exports.memory)) {
      throw createMemoryTypeError(exports.memory);
    }

    this.#memory = exports.memory as WebAssembly.Memory;
    this.#started = true;

    if (typeof exports._initialize === "function") {
      (exports._initialize as Function)();
    }
  }
}

return {
  default: { WASI },
  WASI,
};
})();
