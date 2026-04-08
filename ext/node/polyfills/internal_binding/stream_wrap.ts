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
// - https://github.com/nodejs/node/blob/master/src/stream_base-inl.h
// - https://github.com/nodejs/node/blob/master/src/stream_base.h
// - https://github.com/nodejs/node/blob/master/src/stream_base.cc
// - https://github.com/nodejs/node/blob/master/src/stream_wrap.h
// - https://github.com/nodejs/node/blob/master/src/stream_wrap.cc

import { primordials } from "ext:core/mod.js";
const {
  ObjectPrototypeIsPrototypeOf,
  Int32Array,
  Uint8ArrayPrototype,
} = primordials;

import { TextEncoder } from "ext:deno_web/08_text_encoding.js";
import { Buffer } from "node:buffer";
import { notImplemented } from "ext:deno_node/_utils.ts";
import { HandleWrap } from "ext:deno_node/internal_binding/handle_wrap.ts";
import {
  AsyncWrap,
  providerType,
} from "ext:deno_node/internal_binding/async_wrap.ts";
import { NodeTypeError } from "ext:deno_node/internal/errors.ts";

import { op_stream_base_register_state } from "ext:core/ops";

/** The stream base state array shared with Rust via op_stream_base_register_state. */
export const streamBaseState = new Int32Array(5);

export const kReadBytesOrError = 0;
export const kArrayBufferOffset = 1;
export const kBytesWritten = 2;
export const kLastWriteWasAsync = 3;
export const kNumStreamBaseStateFields = 4;

op_stream_base_register_state(streamBaseState);

export class ShutdownWrap<
  H extends HandleWrap = HandleWrap,
> extends AsyncWrap {
  handle!: H;
  oncomplete!: (status: number) => void;
  callback!: () => void;

  constructor() {
    super(providerType.SHUTDOWNWRAP);
  }
}

export class LibuvStreamWrap extends HandleWrap {
  reading!: boolean;
  destroyed = false;
  writeQueueSize = 0;
  bytesRead = 0;
  bytesWritten = 0;

  onread!:
    | ((_arrayBuffer: Uint8Array, _nread: number) => Uint8Array | undefined)
    | undefined;

  cancelHandle?: number;
  upgrading?: Promise<void>;

  constructor(
    provider: providerType,
    _rid?: number,
  ) {
    super(provider, _rid);
  }

  /**
   * Start the reading of the stream.
   * Subclasses (TCP, Pipe) override this to use native handles.
   * @return An error status code.
   */
  readStart(): number {
    notImplemented("LibuvStreamWrap.prototype.readStart");
  }

  /**
   * Stop the reading of the stream.
   * Subclasses (TCP, Pipe) override this to use native handles.
   * @return An error status code.
   */
  readStop(): number {
    notImplemented("LibuvStreamWrap.prototype.readStop");
  }

  /**
   * Shutdown the stream.
   * Subclasses (TCP, Pipe) override this to use native handles.
   * @param req A shutdown request wrapper.
   * @return An error status code.
   */
  shutdown(req: ShutdownWrap<LibuvStreamWrap>): number {
    const status = this._onClose();

    try {
      req.oncomplete(status);
    } catch {
      // swallow callback error.
    }

    return 0;
  }

  /**
   * @param userBuf
   * @return An error status code.
   */
  useUserBuffer(_userBuf: unknown): number {
    notImplemented("LibuvStreamWrap.prototype.useUserBuffer");
  }

  /**
   * Write a buffer to the stream.
   * Subclasses (TCP, Pipe) override this to use native handles.
   * @param req A write request wrapper.
   * @param data The Uint8Array buffer to write to the stream.
   * @return An error status code.
   */
  writeBuffer(
    _req: WriteWrap<LibuvStreamWrap>,
    data: Uint8Array,
  ): number {
    if (!ObjectPrototypeIsPrototypeOf(Uint8ArrayPrototype, data)) {
      throw new NodeTypeError(
        "ERR_INVALID_ARG_TYPE",
        "Second argument must be a buffer",
      );
    }

    notImplemented("LibuvStreamWrap.prototype.writeBuffer");
  }

  /**
   * Write multiple chunks at once.
   * Subclasses (TCP, Pipe) override this to use native handles.
   * @param req A write request wrapper.
   * @param chunks
   * @param allBuffers
   * @return An error status code.
   */
  writev(
    _req: WriteWrap<LibuvStreamWrap>,
    _chunks: Buffer[] | (string | Buffer)[],
    _allBuffers: boolean,
  ): number {
    notImplemented("LibuvStreamWrap.prototype.writev");
  }

  /**
   * Write an ASCII string to the stream.
   * @return An error status code.
   */
  writeAsciiString(req: WriteWrap<LibuvStreamWrap>, data: string): number {
    const buffer = new TextEncoder().encode(data);
    return this.writeBuffer(req, buffer);
  }

  /**
   * Write an UTF8 string to the stream.
   * @return An error status code.
   */
  writeUtf8String(req: WriteWrap<LibuvStreamWrap>, data: string): number {
    const buffer = new TextEncoder().encode(data);
    return this.writeBuffer(req, buffer);
  }

  /**
   * Write an UCS2 string to the stream.
   * @return An error status code.
   */
  writeUcs2String(_req: WriteWrap<LibuvStreamWrap>, _data: string): number {
    notImplemented("LibuvStreamWrap.prototype.writeUcs2String");
  }

  /**
   * Write an LATIN1 string to the stream.
   * @return An error status code.
   */
  writeLatin1String(req: WriteWrap<LibuvStreamWrap>, data: string): number {
    const buffer = Buffer.from(data, "latin1");
    return this.writeBuffer(req, buffer);
  }

  override _onClose(): number {
    return 0;
  }
}

export class WriteWrap<
  H extends LibuvStreamWrap = LibuvStreamWrap,
> extends AsyncWrap {
  handle!: H;
  oncomplete!: (status: number) => void;
  async!: boolean;
  bytes!: number;
  buffer!: unknown;
  callback!: unknown;
  _chunks!: unknown;

  constructor() {
    super(providerType.WRITEWRAP);
  }
}

export function afterWriteDispatched(
  req: WriteWrap<LibuvStreamWrap>,
  _handle: LibuvStreamWrap,
  // deno-lint-ignore no-explicit-any
  cb: any,
) {
  req.async = streamBaseState[kLastWriteWasAsync] !== 0;
  if (!req.async) {
    cb(req);
  }
}

export function writevGeneric(
  // deno-lint-ignore no-explicit-any
  _owner: any,
  // deno-lint-ignore no-explicit-any
  _data: any,
  // deno-lint-ignore no-explicit-any
  _cb: any,
) {
  notImplemented("writevGeneric");
}

export function writeGeneric(
  // deno-lint-ignore no-explicit-any
  _owner: any,
  // deno-lint-ignore no-explicit-any
  _data: any,
  // deno-lint-ignore no-explicit-any
  _cb: any,
) {
  notImplemented("writeGeneric");
}
