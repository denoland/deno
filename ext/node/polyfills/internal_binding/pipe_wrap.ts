// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent, Inc. and other Node contributors.
//
// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the
// "Software"), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to permit
// persons to whom the Software is furnished to do so, subject to the
// following conditions:
//
// The above copyright notice and this permission notice shall be included
// in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
// OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN
// NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
// DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
// OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE
// USE OR OTHER DEALINGS IN THE SOFTWARE.

// This module ports:
// - https://github.com/nodejs/node/blob/master/src/pipe_wrap.cc
// - https://github.com/nodejs/node/blob/master/src/pipe_wrap.h

import { core, primordials } from "ext:core/mod.js";
import {
  op_node_create_pipe,
  op_node_fd_set_blocking,
  op_node_fs_close,
  op_node_fs_read_deferred,
  op_node_fs_read_sync,
  op_node_fs_write_deferred,
  op_node_fs_write_sync,
  op_node_register_fd,
  op_pipe_connect,
  op_pipe_open,
  op_pipe_windows_wait,
} from "ext:core/ops";
import { PipeConn } from "ext:deno_net/01_net.js";

const { internalRidSymbol } = core;
import { ConnectionWrap } from "ext:deno_node/internal_binding/connection_wrap.ts";
import {
  AsyncWrap,
  providerType,
} from "ext:deno_node/internal_binding/async_wrap.ts";
import { LibuvStreamWrap } from "ext:deno_node/internal_binding/stream_wrap.ts";
import { codeMap } from "ext:deno_node/internal_binding/uv.ts";
import { delay } from "ext:deno_node/_util/async.ts";
import { kStreamBaseField } from "ext:deno_node/internal_binding/stream_wrap.ts";
import {
  ceilPowOf2,
  INITIAL_ACCEPT_BACKOFF_DELAY,
  MAX_ACCEPT_BACKOFF_DELAY,
} from "ext:deno_node/internal_binding/_listen.ts";
import { isWindows } from "ext:deno_node/_util/os.ts";
import { fs } from "ext:deno_node/internal_binding/constants.ts";

const {
  Error,
  ErrorPrototype,
  FunctionPrototypeCall,
  MapPrototypeGet,
  ObjectDefineProperty,
  ObjectPrototypeIsPrototypeOf,
  PromisePrototypeThen,
  ReflectHas,
  StringPrototypeIncludes,
  queueMicrotask,
} = primordials;

export enum socketType {
  SOCKET,
  SERVER,
  IPC,
}

/**
 * StreamBase implementation backed by a raw OS file descriptor.
 * Uses the NodeFsState fd-based ops for I/O, matching how Node.js
 * uses libuv streams on raw fds via uv_pipe_open().
 */
class FdStreamBase {
  #fd: number;
  #closed = false;
  #blocking = false;
  #pendingRead: Promise<number> | null = null;
  #isRefed = true;

  constructor(fd: number) {
    this.#fd = fd;
    ObjectDefineProperty(this, internalRidSymbol, {
      __proto__: null,
      enumerable: false,
      value: fd,
    });
  }

  async read(buf: Uint8Array): Promise<number | null> {
    if (this.#closed) return null;
    if (this.#blocking) {
      const nread = op_node_fs_read_sync(this.#fd, buf, -1);
      return nread === 0 ? null : nread;
    }
    // position = -1 means non-positioned (sequential) read
    const promise = op_node_fs_read_deferred(this.#fd, buf, -1);
    this.#pendingRead = promise;
    if (!this.#isRefed) {
      core.unrefOpPromise(promise);
    }
    try {
      const nread = await promise;
      return nread === 0 ? null : nread;
    } finally {
      this.#pendingRead = null;
    }
  }

  async write(data: Uint8Array): Promise<number> {
    if (this.#blocking) {
      return op_node_fs_write_sync(this.#fd, data, -1);
    }
    // position = -1 means non-positioned (sequential) write
    return await op_node_fs_write_deferred(this.#fd, data, -1);
  }

  close(): void {
    if (!this.#closed) {
      this.#closed = true;
      op_node_fs_close(this.#fd);
    }
  }

  ref(): void {
    this.#isRefed = true;
    if (this.#pendingRead) {
      core.refOpPromise(this.#pendingRead);
    }
  }

  unref(): void {
    this.#isRefed = false;
    if (this.#pendingRead) {
      core.unrefOpPromise(this.#pendingRead);
    }
  }

  setBlocking(enable: boolean): number {
    // On Unix, toggle O_NONBLOCK on the fd.
    // On Windows, the op is a no-op but we track the flag so
    // read/write use sync ops instead (matching Node.js behavior
    // of making stdout/stderr blocking on Windows pipes).
    const err = op_node_fd_set_blocking(this.#fd, enable);
    if (err === 0) {
      this.#blocking = enable;
    }
    return err;
  }
}

export class Pipe extends ConnectionWrap {
  override reading = false;
  ipc: boolean;

  // REF: https://github.com/nodejs/node/blob/master/deps/uv/src/win/pipe.c#L48
  #pendingInstances = 4;

  #address?: string;

  #backlog?: number;
  #listener!: Deno.Listener;
  #connections = 0;

  #closed = false;
  #acceptBackoffDelay?: number;
  #serverPipeRid?: number;

  constructor(type: number, conn?: Deno.UnixConn | StreamBase) {
    let provider: providerType;
    let ipc: boolean;

    switch (type) {
      case socketType.SOCKET: {
        provider = providerType.PIPEWRAP;
        ipc = false;

        break;
      }
      case socketType.SERVER: {
        provider = providerType.PIPESERVERWRAP;
        ipc = false;

        break;
      }
      case socketType.IPC: {
        provider = providerType.PIPEWRAP;
        ipc = true;

        break;
      }
      default: {
        throw new Error("Unreachable code");
      }
    }

    super(provider, conn);

    this.ipc = ipc;

    if (
      conn && provider === providerType.PIPEWRAP &&
      ReflectHas(conn, "localAddr")
    ) {
      const localAddr = conn.localAddr;
      this.#address = localAddr.path;
    }
  }

  open(fd: number): number {
    try {
      // Register the fd in NodeFsState and use fd-based I/O.
      // This handles pipes, PTYs, FIFOs, sockets on both Unix and Windows,
      // matching libuv's uv_pipe_open() which accepts any stream-like fd.
      op_node_register_fd(fd);
      this[kStreamBaseField] = new FdStreamBase(fd);
      return 0;
    } catch (e) {
      if (
        ObjectPrototypeIsPrototypeOf(ErrorPrototype, e) &&
        ReflectHas(e as Error, "code")
      ) {
        return MapPrototypeGet(codeMap, (e as { code: string }).code) ??
          MapPrototypeGet(codeMap, "UNKNOWN")!;
      }
      return MapPrototypeGet(codeMap, "UNKNOWN")!;
    }
  }

  setBlocking(enable: boolean): number {
    const stream = this[kStreamBaseField];
    if (stream && ReflectHas(stream, "setBlocking")) {
      return (stream as FdStreamBase).setBlocking(enable);
    }
    return 0;
  }

  /**
   * Bind to a Unix domain or Windows named pipe.
   * @param name Unix domain or Windows named pipe the server should listen to.
   * @return An error status code.
   */
  bind(name: string) {
    // Deno doesn't currently separate bind from connect. For now we noop under
    // the assumption we will connect shortly.
    // REF: https://doc.deno.land/deno/unstable/~/Deno.connect

    this.#address = name;

    return 0;
  }

  /**
   * Internal Windows pipe connect with retry for ERROR_PIPE_BUSY.
   * Node.js handles this transparently via WaitNamedPipeW in libuv.
   * We emulate this by retrying with a short delay.
   */
  #connectWindows(
    req: PipeConnectWrap,
    address: string,
    attempt: number,
  ) {
    try {
      const rid = op_pipe_connect(
        address,
        true,
        true,
        "net.createConnection()",
      );
      this[kStreamBaseField] = new PipeConn(rid);
      this.#address = req.address = address;

      queueMicrotask(() => {
        try {
          this.afterConnect(req, 0);
        } catch {
          // swallow callback errors.
        }
      });
    } catch (e: unknown) {
      const err = e as {
        code?: string;
        message?: string;
      };
      const msg = err.message ?? "";

      // ERROR_PIPE_BUSY (231): All pipe instances are currently in use.
      // Node.js/libuv handles this via WaitNamedPipeW(30000) which blocks
      // up to 30 seconds. We emulate this by polling with retries:
      // 300 attempts x 100ms = 30s max wait, matching libuv's timeout.
      if (err.code === "EBUSY" && attempt < 300) {
        setTimeout(() => {
          this.#connectWindows(req, address, attempt + 1);
        }, 100);
        return;
      }

      // Map other errors to UV error codes
      let code;
      if (err.code !== undefined) {
        code = MapPrototypeGet(codeMap, err.code) ??
          MapPrototypeGet(codeMap, "UNKNOWN")!;
      } else {
        if (StringPrototypeIncludes(msg, "ENOTSOCK")) {
          code = MapPrototypeGet(codeMap, "ENOTSOCK")!;
        } else if (
          StringPrototypeIncludes(msg, "ENOENT") ||
          StringPrototypeIncludes(msg, "NotFound")
        ) {
          code = MapPrototypeGet(codeMap, "ENOENT")!;
        } else {
          code = MapPrototypeGet(codeMap, "UNKNOWN")!;
        }
      }

      queueMicrotask(() => {
        try {
          this.afterConnect(req, code);
        } catch {
          // swallow callback errors.
        }
      });
    }
  }

  /**
   * Connect to a Unix domain or Windows named pipe.
   * @param req A PipeConnectWrap instance.
   * @param address Unix domain or Windows named pipe the server should connect to.
   * @return An error status code.
   */
  connect(req: PipeConnectWrap, address: string) {
    if (isWindows) {
      this.#connectWindows(req, address, 0);
      return 0;
    }

    const connectOptions: Deno.UnixConnectOptions = {
      path: address,
      transport: "unix",
    };

    PromisePrototypeThen(
      Deno.connect(connectOptions),
      (conn: Deno.UnixConn) => {
        const localAddr = conn.localAddr as Deno.UnixAddr;

        this.#address = req.address = localAddr.path;
        this[kStreamBaseField] = conn;

        try {
          this.afterConnect(req, 0);
        } catch {
          // swallow callback errors.
        }
      },
      (e) => {
        const code = MapPrototypeGet(codeMap, e.code ?? "UNKNOWN") ??
          MapPrototypeGet(codeMap, "UNKNOWN")!;

        try {
          this.afterConnect(req, code);
        } catch {
          // swallow callback errors.
        }
      },
    );

    return 0;
  }

  /**
   * Listen for new connections.
   * @param backlog The maximum length of the queue of pending connections.
   * @return An error status code.
   */
  listen(backlog: number): number {
    this.#backlog = isWindows
      ? this.#pendingInstances
      : ceilPowOf2(backlog + 1);

    if (isWindows) {
      try {
        const rid = op_pipe_open(
          this.#address!,
          this.#pendingInstances,
          false,
          true,
          true,
          "net.Server.listen()",
        );

        this.#serverPipeRid = rid;
        this.#acceptWindows();

        return 0;
      } catch (e) {
        if (ObjectPrototypeIsPrototypeOf(Deno.errors.NotCapable.prototype, e)) {
          throw e;
        }
        return MapPrototypeGet(codeMap, e.code ?? "UNKNOWN") ??
          MapPrototypeGet(codeMap, "UNKNOWN")!;
      }
    }

    const listenOptions = {
      path: this.#address!,
      transport: "unix" as const,
    };

    let listener;

    try {
      listener = Deno.listen(listenOptions);
    } catch (e) {
      if (ObjectPrototypeIsPrototypeOf(Deno.errors.NotCapable.prototype, e)) {
        throw e;
      }
      return MapPrototypeGet(codeMap, e.code ?? "UNKNOWN") ??
        MapPrototypeGet(codeMap, "UNKNOWN")!;
    }

    const address = listener.addr as Deno.UnixAddr;
    this.#address = address.path;

    this.#listener = listener;
    this.#accept();

    return 0;
  }

  override ref() {
    if (this.#listener) {
      this.#listener.ref();
    }
    const stream = this[kStreamBaseField];
    if (stream && typeof stream.ref === "function") {
      stream.ref();
    }
  }

  override unref() {
    if (this.#listener) {
      this.#listener.unref();
    }
    const stream = this[kStreamBaseField];
    if (stream && typeof stream.unref === "function") {
      stream.unref();
    }
  }

  /**
   * Set the number of pending pipe instance handles when the pipe server is
   * waiting for connections. This setting applies to Windows only.
   * @param instances Number of pending pipe instances.
   */
  setPendingInstances(instances: number) {
    this.#pendingInstances = instances;
  }

  /**
   * Alters pipe permissions, allowing it to be accessed from processes run by
   * different users. Makes the pipe writable or readable by all users. Mode
   * can be `UV_WRITABLE`, `UV_READABLE` or `UV_WRITABLE | UV_READABLE`. This
   * function is blocking.
   * @param mode Pipe permissions mode.
   * @return An error status code.
   */
  fchmod(mode: number) {
    if (
      mode != constants.UV_READABLE &&
      mode != constants.UV_WRITABLE &&
      mode != (constants.UV_WRITABLE | constants.UV_READABLE)
    ) {
      return MapPrototypeGet(codeMap, "EINVAL");
    }

    let desiredMode = 0;

    if (mode & constants.UV_READABLE) {
      desiredMode |= fs.S_IRUSR | fs.S_IRGRP | fs.S_IROTH;
    }
    if (mode & constants.UV_WRITABLE) {
      desiredMode |= fs.S_IWUSR | fs.S_IWGRP | fs.S_IWOTH;
    }

    // TODO(cmorten): this will incorrectly throw on Windows
    // REF: https://github.com/denoland/deno/issues/4357
    try {
      Deno.chmodSync(this.#address!, desiredMode);
    } catch {
      // TODO(cmorten): map errors to appropriate error codes.
      return MapPrototypeGet(codeMap, "UNKNOWN")!;
    }

    return 0;
  }

  /** Calculate and apply backoff delay following an unsuccessful accept. */
  async #acceptBackoff(): Promise<void> {
    // Backoff after transient errors to allow time for the system to
    // recover, and avoid blocking up the event loop with a continuously
    // running loop.
    if (!this.#acceptBackoffDelay) {
      this.#acceptBackoffDelay = INITIAL_ACCEPT_BACKOFF_DELAY;
    } else {
      this.#acceptBackoffDelay *= 2;
    }

    if (this.#acceptBackoffDelay >= MAX_ACCEPT_BACKOFF_DELAY) {
      this.#acceptBackoffDelay = MAX_ACCEPT_BACKOFF_DELAY;
    }

    await delay(this.#acceptBackoffDelay);
  }

  /** Accept new connections on Windows named pipes. */
  async #acceptWindows(): Promise<void> {
    while (!this.#closed) {
      try {
        // Wait for a client to connect
        await op_pipe_windows_wait(this.#serverPipeRid!);

        // Connection established, wrap it
        const connectionHandle = new Pipe(socketType.SOCKET);
        connectionHandle[kStreamBaseField] = new PipeConn(this.#serverPipeRid!);

        this.#connections++;

        try {
          this.onconnection!(0, connectionHandle);
        } catch {
          // swallow callback errors.
        }

        // Reset the backoff delay upon successful accept.
        this.#acceptBackoffDelay = undefined;

        // Create a new server pipe for the next connection
        const newRid = op_pipe_open(
          this.#address!,
          this.#pendingInstances,
          false,
          true,
          true,
          "net.Server.listen()",
        );

        this.#serverPipeRid = newRid;
      } catch {
        if (this.#closed) {
          return;
        }

        try {
          this.onconnection!(MapPrototypeGet(codeMap, "UNKNOWN")!, undefined);
        } catch {
          // swallow callback errors.
        }

        await delay(this.#acceptBackoffDelay || INITIAL_ACCEPT_BACKOFF_DELAY);
      }
    }
  }

  /** Accept new connections. */
  async #accept(): Promise<void> {
    while (!this.#closed) {
      if (this.#connections > this.#backlog!) {
        await this.#acceptBackoff();
        continue;
      }

      let connection: Deno.Conn;

      try {
        connection = await this.#listener.accept();
      } catch (e) {
        if (
          ObjectPrototypeIsPrototypeOf(Deno.errors.BadResource.prototype, e) &&
          this.#closed
        ) {
          // Listener and server has closed.
          return;
        }

        try {
          // TODO(cmorten): map errors to appropriate error codes.
          this.onconnection!(MapPrototypeGet(codeMap, "UNKNOWN")!, undefined);
        } catch {
          // swallow callback errors.
        }

        await this.#acceptBackoff();
        continue;
      }

      // Reset the backoff delay upon successful accept.
      this.#acceptBackoffDelay = undefined;

      const connectionHandle = new Pipe(socketType.SOCKET, connection);
      this.#connections++;

      try {
        this.onconnection!(0, connectionHandle);
      } catch {
        // swallow callback errors.
      }
    }
  }

  /** Handle server closure. */
  override _onClose(): number {
    this.#closed = true;
    this.reading = false;

    this.#address = undefined;

    this.#backlog = undefined;
    this.#connections = 0;
    this.#acceptBackoffDelay = undefined;

    if (this.provider === providerType.PIPESERVERWRAP) {
      if (this.#serverPipeRid !== undefined) {
        core.tryClose(this.#serverPipeRid);
        this.#serverPipeRid = undefined;
      }

      try {
        this.#listener.close();
      } catch {
        // listener already closed
      }
    }

    return FunctionPrototypeCall(LibuvStreamWrap.prototype._onClose, this);
  }
}

export class PipeConnectWrap extends AsyncWrap {
  oncomplete!: (
    status: number,
    handle: ConnectionWrap,
    req: PipeConnectWrap,
    readable: boolean,
    writeable: boolean,
  ) => void;
  address!: string;

  constructor() {
    super(providerType.PIPECONNECTWRAP);
  }
}

export enum constants {
  SOCKET = socketType.SOCKET,
  SERVER = socketType.SERVER,
  IPC = socketType.IPC,
  UV_READABLE = 1,
  UV_WRITABLE = 2,
}

/** Create an anonymous pipe pair. Returns [readFd, writeFd]. */
export function createPipe(): [number, number] {
  return op_node_create_pipe();
}
