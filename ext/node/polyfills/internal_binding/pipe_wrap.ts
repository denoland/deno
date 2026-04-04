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

import { primordials } from "ext:core/mod.js";
import {
  NativePipe as NativePipeHandle,
  op_node_create_pipe,
} from "ext:core/ops";

import { Buffer } from "node:buffer";
import { ConnectionWrap } from "ext:deno_node/internal_binding/connection_wrap.ts";
import {
  AsyncWrap,
  providerType,
} from "ext:deno_node/internal_binding/async_wrap.ts";
import {
  kArrayBufferOffset,
  kReadBytesOrError,
  LibuvStreamWrap,
  streamBaseState,
  WriteWrap,
} from "ext:deno_node/internal_binding/stream_wrap.ts";
import { codeMap } from "ext:deno_node/internal_binding/uv.ts";
import { ceilPowOf2 } from "ext:deno_node/internal_binding/_listen.ts";
import { fs } from "ext:deno_node/internal_binding/constants.ts";

const {
  Error,
  FunctionPrototypeCall,
  MapPrototypeGet,
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
  }

  get fd(): number {
    return this.#native?.fd ?? -1;
  }

  open(fd: number): number {
    this.#ensureNative();
    // NativePipe.open checks FdTable for duplicates and registers
    // as UvOwned (the native handle owns the fd, not FdTable).
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
          // deno-lint-ignore prefer-primordials
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

  override writev(
    req: WriteWrap<LibuvStreamWrap>,
    chunks: Buffer[] | (string | Buffer)[],
    allBuffers: boolean,
  ): number {
    if (this.#native) {
      // Concat all chunks into a single buffer and use writeBuffer.
      const count = allBuffers ? chunks.length : chunks.length >> 1;
      // deno-lint-ignore prefer-primordials
      const buffers: Buffer[] = new Array(count);
      if (!allBuffers) {
        for (let i = 0; i < count; i++) {
          const chunk = chunks[i * 2];
          if (Buffer.isBuffer(chunk)) {
            buffers[i] = chunk;
          } else {
            const encoding: string = chunks[i * 2 + 1] as string;
            buffers[i] = Buffer.from(chunk as string, encoding);
          }
        }
      } else {
        for (let i = 0; i < count; i++) {
          buffers[i] = chunks[i] as Buffer;
        }
      }
      // deno-lint-ignore prefer-primordials
      return this.writeBuffer(req, Buffer.concat(buffers));
    }
    return super.writev(req, chunks, allBuffers);
  }

  setBlocking(enable: boolean): number {
    if (this.#native) {
      return this.#native.setBlocking(enable ? 1 : 0);
    }
    return 0;
  }

  bind(name: string) {
    this.#ensureNative();
    return this.#native.pipeBindToPath(name);
  }

  connect(req: PipeConnectWrap, address: string) {
    this.#ensureNative();
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
    this.#ensureNative();
    this.#native.setPendingInstances(instances);
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

    if (this.#native) {
      return this.#native.fchmod(desiredMode);
    }
    return 0;
  }

  /** Handle server closure. */
  override _onClose(): number {
    this.reading = false;

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
