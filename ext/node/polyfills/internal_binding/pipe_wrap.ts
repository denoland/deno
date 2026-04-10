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
  kLastWriteWasAsync,
  kReadBytesOrError,
  LibuvStreamWrap,
  ShutdownWrap,
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

  // deno-lint-ignore no-explicit-any
  #native: any;
  #closed = false;

  constructor(type: number) {
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

    super(provider);
    this.ipc = ipc;
    this.#native = new NativePipeHandle(
      ipc ? socketType.IPC : socketType.SOCKET,
    );
    this.#native.setOwner();
  }

  get fd(): number {
    return this.#native.fd;
  }

  open(fd: number): number {
    return this.#native.open(fd);
  }

  override readStart(): number {
    if (this.#closed) {
      return 0;
    }
    this.reading = true;
    // deno-lint-ignore no-this-alias
    const self = this;
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

  override readStop(): number {
    return this.#native.readStop();
  }

  override writeBuffer(
    req: WriteWrap<LibuvStreamWrap>,
    data: Uint8Array,
  ): number {
    const ret = this.#native.writeBuffer(data);
    if (ret !== 0) {
      // uv_write failed to queue; report synchronously.
      streamBaseState[kLastWriteWasAsync] = 0;
      return ret;
    }
    streamBaseState[kLastWriteWasAsync] = 1;
    // The actual write result (including EPIPE) arrives asynchronously
    // via the native onwrite callback.
    this.#native.onwrite = (status: number) => {
      this.#native.onwrite = undefined;
      try {
        req.oncomplete(status);
      } catch {
        // swallow callback errors.
      }
    };
    return 0;
  }

  override writev(
    req: WriteWrap<LibuvStreamWrap>,
    chunks: Buffer[] | (string | Buffer)[],
    allBuffers: boolean,
  ): number {
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

  override shutdown(req: ShutdownWrap<LibuvStreamWrap>): number {
    // deno-lint-ignore no-this-alias
    const self = this;
    this.#native.onshutdown = function (status: number) {
      self.#native.onshutdown = undefined;
      try {
        req.oncomplete(status);
      } catch {
        // swallow callback errors.
      }
    };
    const ret = this.#native.shutdown();
    if (ret !== 0) {
      // Native shutdown failed (e.g. ENOTCONN), fire oncomplete via microtask.
      this.#native.onshutdown = undefined;
      queueMicrotask(() => {
        try {
          req.oncomplete(
            MapPrototypeGet(codeMap, "UNKNOWN") ?? -4094,
          );
        } catch {
          // swallow callback errors.
        }
      });
    }
    return 0;
  }

  setBlocking(enable: boolean): number {
    return this.#native.setBlocking(enable ? 1 : 0);
  }

  bind(name: string) {
    return this.#native.pipeBindToPath(name);
  }

  connect(req: PipeConnectWrap, address: string) {
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
    // deno-lint-ignore no-this-alias
    const self = this;
    this.#native.onconnection = function (status: number) {
      if (status === 0) {
        const clientHandle = new Pipe(socketType.SOCKET);
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

  override ref() {
    this.#native.ref();
  }

  override unref() {
    this.#native.unref();
  }

  setPendingInstances(instances: number) {
    this.#native.setPendingInstances(instances);
  }

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

    return this.#native.fchmod(desiredMode);
  }

  override _onClose(): number {
    this.reading = false;
    this.#closed = true;
    this.#native.close();
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
