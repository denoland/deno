// Copyright 2018-2026 the Deno authors. MIT license.

import { primordials } from "ext:core/mod.js";
import { WasiContext } from "ext:core/ops";

const {
  ArrayIsArray,
  Error,
  ObjectEntries,
  TypeError,
} = primordials;

class WASIProcExit {
  code: number;
  constructor(code: number) {
    this.code = code;
  }
}

class WASI {
  #ctx;
  #version: string;
  #started = false;
  #returnOnExit: boolean;
  #wasiImport;

  constructor(options: {
    args?: string[];
    env?: Record<string, string>;
    preopens?: Record<string, string>;
    returnOnExit?: boolean;
    stdin?: number;
    stdout?: number;
    stderr?: number;
    version: string;
  } = { version: "preview1" }) {
    if (options.version !== "preview1" && options.version !== "unstable") {
      throw new TypeError(
        `"${options.version}" is not a valid WASI version. Supported versions: "preview1", "unstable"`,
      );
    }

    const args = options.args ?? [];
    if (!ArrayIsArray(args)) {
      throw new TypeError("options.args must be an array");
    }

    const envObj = options.env ?? {};
    const envPairs: [string, string][] = [];
    for (const [key, value] of ObjectEntries(envObj)) {
      if (typeof value === "string") {
        envPairs.push([key, value]);
      }
    }

    const preopens: [string, string][] = [];
    if (options.preopens) {
      for (const [virtualPath, realPath] of ObjectEntries(options.preopens)) {
        if (typeof realPath === "string") {
          preopens.push([virtualPath, realPath]);
        }
      }
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
        throw new WASIProcExit(exitCode);
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
      throw new Error("WASI instance has not been started");
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

  start(instance: WebAssembly.Instance): number {
    if (this.#started) {
      throw new Error("WASI instance has already started");
    }

    const exports = instance.exports;
    if (typeof exports._initialize === "function") {
      throw new Error(
        "This instance contains a _initialize export and should be initialized with initialize(), not start()",
      );
    }
    if (typeof exports._start !== "function") {
      throw new Error("Instance does not have a _start export");
    }
    if (!(exports.memory instanceof WebAssembly.Memory)) {
      throw new TypeError("Instance must export a memory named 'memory'");
    }

    this.#memory = exports.memory;
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

  initialize(instance: WebAssembly.Instance): void {
    if (this.#started) {
      throw new Error("WASI instance has already started");
    }

    const exports = instance.exports;
    if (typeof exports._start === "function") {
      throw new Error(
        "This instance contains a _start export and should be started with start(), not initialize()",
      );
    }
    if (!(exports.memory instanceof WebAssembly.Memory)) {
      throw new TypeError("Instance must export a memory named 'memory'");
    }

    this.#memory = exports.memory;
    this.#started = true;

    if (typeof exports._initialize === "function") {
      (exports._initialize as Function)();
    }
  }

  finalizeBindings(
    instance: WebAssembly.Instance,
    options?: { memory?: WebAssembly.Memory },
  ): void {
    if (this.#started) {
      throw new Error("WASI instance has already started");
    }

    const memory = options?.memory ?? instance.exports.memory;
    if (!(memory instanceof WebAssembly.Memory)) {
      throw new TypeError(
        "A valid WebAssembly.Memory must be provided or exported",
      );
    }

    this.#memory = memory;
    this.#started = true;
  }
}

export { WASI };
export default { WASI };
