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
  NativePipe as NativePipeHandle,
  op_node_create_pipe,
  op_pipe_connect,
  op_pipe_open,
  op_pipe_windows_wait,
} from "ext:core/ops";
import { PipeConn } from "ext:deno_net/01_net.js";

import { ConnectionWrap } from "ext:deno_node/internal_binding/connection_wrap.ts";
import {
  AsyncWrap,
  providerType,
} from "ext:deno_node/internal_binding/async_wrap.ts";
import {
  kArrayBufferOffset,
  kReadBytesOrError,
  kStreamBaseField,
  LibuvStreamWrap,
  streamBaseState,
  WriteWrap,
} from "ext:deno_node/internal_binding/stream_wrap.ts";
import { codeMap } from "ext:deno_node/internal_binding/uv.ts";
import { delay } from "ext:deno_node/_util/async.ts";
import {
  ceilPowOf2,
  INITIAL_ACCEPT_BACKOFF_DELAY,
  MAX_ACCEPT_BACKOFF_DELAY,
} from "ext:deno_node/internal_binding/_listen.ts";
import { isWindows } from "ext:deno_node/_util/os.ts";
import { fs } from "ext:deno_node/internal_binding/constants.ts";

const {
  Error,
  FunctionPrototypeCall,
  MapPrototypeGet,
  ObjectPrototypeIsPrototypeOf,
  ReflectHas,
  StringPrototypeIncludes,
  queueMicrotask,
} = primordials;

export enum socketType {
  SOCKET,
  SERVER,
  IPC,
}

export class Pipe extends ConnectionWrap {
  override reading = false;
  ipc: boolean;

  #pendingInstances = 4; // Windows only
  #address?: string;
  #closed = false;
  #acceptBackoffDelay?: number;
  #serverPipeRid?: number; // Windows only
  #connections = 0; // Windows only

  // Native pipe handle for fd-based I/O (via uv_pipe_open).
  // When set, readStart/readStop/writeBuffer/close delegate to native.
  // deno-lint-ignore no-explicit-any
  #native: any = null;

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
    // Use the native uv_pipe_t handle for fd-based I/O.
    // This integrates with the event loop directly, providing proper
    // cancellation on close and ref/unref semantics.
    this.#native = new NativePipeHandle(
      this.ipc ? socketType.IPC : socketType.SOCKET,
    );
    const err = this.#native.open(fd);
    if (err !== 0) {
      this.#native = null;
      return err;
    }
    return 0;
  }

  override readStart(): number {
    if (this.#native) {
      this.reading = true;
      // deno-lint-ignore no-this-alias
      const self = this;
      // stream_read_cb fires onread(nread, buf) on the NativePipe JS object.
      // We wrap it to: update streamBaseState, swap arg order, and route
      // through the Pipe wrapper's onread (matching TCP's pattern).
      this.#native.onread = function (
        nread: number,
        buf: Uint8Array | undefined,
      ) {
        streamBaseState[kReadBytesOrError] = nread;
        if (nread > 0) {
          self.bytesRead += nread;
        }
        streamBaseState[kArrayBufferOffset] = 0;
        try {
          self.onread!(buf ?? new Uint8Array(0), nread);
        } catch {
          // swallow callback errors.
        }
        if (nread < 0) {
          self.reading = false;
        }
      };
      return this.#native.readStart();
    }
    return super.readStart();
  }

  override readStop(): number {
    if (this.#native) {
      return this.#native.readStop();
    }
    return super.readStop();
  }

  override writeBuffer(
    req: WriteWrap<LibuvStreamWrap>,
    data: Uint8Array,
  ): number {
    if (this.#native) {
      const ret = this.#native.writeBuffer(data);
      // Fire completion callback asynchronously, matching Node.js behavior.
      queueMicrotask(() => {
        try {
          req.oncomplete(ret === 0 ? 0 : MapPrototypeGet(codeMap, "UNKNOWN")!);
        } catch {
          // swallow callback errors.
        }
      });
      return 0;
    }
    return super.writeBuffer(req, data);
  }

  setBlocking(enable: boolean): number {
    if (this.#native) {
      return this.#native.setBlocking(enable ? 1 : 0);
    }
    return 0;
  }

  bind(name: string) {
    this.#address = name;
    if (!isWindows) {
      this.#ensureNative();
      return this.#native.pipeBindToPath(name);
    }
    return 0;
  }

  connect(req: PipeConnectWrap, address: string) {
    if (isWindows) {
      this.#connectWindows(req, address, 0);
      return 0;
    }
    this.#ensureNative();
    this.#address = address;
    // Set the connect callback on the native handle so connect_cb can find it.
    // deno-lint-ignore no-this-alias
    const self = this;
    this.#native.onconnect = function (status: number) {
      try {
        self.afterConnect(req, status);
      } catch {
        // swallow callback errors.
      }
    };
    return this.#native.connect(address);
  }

  listen(backlog: number): number {
    if (isWindows) {
      return this.#listenWindows();
    }
    this.#ensureNative();
    // Set the connection callback on the native handle so
    // server_connection_cb can find it.
    // deno-lint-ignore no-this-alias
    const self = this;
    this.#native.onconnection = function (status: number) {
      if (status === 0) {
        // Accept the connection into a new NativePipe.
        const clientHandle = new Pipe(socketType.SOCKET);
        clientHandle.#ensureNative();
        const acceptErr = self.#native.accept(clientHandle.#native);
        if (acceptErr !== 0) {
          try {
            self.onconnection!(acceptErr, undefined);
          } catch {
            // swallow callback errors.
          }
          return;
        }
        try {
          self.onconnection!(0, clientHandle);
        } catch {
          // swallow callback errors.
        }
      } else {
        try {
          self.onconnection!(status, undefined);
        } catch {
          // swallow callback errors.
        }
      }
    };
    return this.#native.listen(ceilPowOf2(backlog + 1));
  }

  /** Ensure a NativePipe handle exists (created lazily). */
  #ensureNative() {
    if (!this.#native) {
      this.#native = new NativePipeHandle(
        this.ipc ? socketType.IPC : socketType.SOCKET,
      );
      // Store the JS object reference so C callbacks can find
      // onconnect/onconnection/onread on it.
      this.#native.setOwner();
    }
  }

  /** Windows: connect with retry for ERROR_PIPE_BUSY. */
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
      const err = e as { code?: string; message?: string };
      const msg = err.message ?? "";
      if (err.code === "EBUSY" && attempt < 300) {
        setTimeout(() => {
          this.#connectWindows(req, address, attempt + 1);
        }, 100);
        return;
      }
      let code;
      if (err.code !== undefined) {
        code = MapPrototypeGet(codeMap, err.code) ??
          MapPrototypeGet(codeMap, "UNKNOWN")!;
      } else if (StringPrototypeIncludes(msg, "ENOTSOCK")) {
        code = MapPrototypeGet(codeMap, "ENOTSOCK")!;
      } else if (
        StringPrototypeIncludes(msg, "ENOENT") ||
        StringPrototypeIncludes(msg, "NotFound")
      ) {
        code = MapPrototypeGet(codeMap, "ENOENT")!;
      } else {
        code = MapPrototypeGet(codeMap, "UNKNOWN")!;
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

  /** Windows: listen using Deno pipe ops. */
  #listenWindows(): number {
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

  override ref() {
    if (this.#native) {
      this.#native.ref();
    }
  }

  override unref() {
    if (this.#native) {
      this.#native.unref();
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

  /** Handle server closure. */
  override _onClose(): number {
    this.#closed = true;
    this.reading = false;
    this.#address = undefined;
    this.#acceptBackoffDelay = undefined;

    // Windows: close server pipe resource
    if (this.#serverPipeRid !== undefined) {
      core.tryClose(this.#serverPipeRid);
      this.#serverPipeRid = undefined;
    }

    // Native handle close is handled by LibUvStreamWrap._onClose
    // which calls uv_close -> close_pipe.
    if (this.#native) {
      this.#native.close();
      this.#native = null;
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
