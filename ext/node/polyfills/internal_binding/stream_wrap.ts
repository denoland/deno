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
  Int32Array,
} = primordials;

import { HandleWrap } from "ext:deno_node/internal_binding/handle_wrap.ts";
import {
  AsyncWrap,
  providerType,
} from "ext:deno_node/internal_binding/async_wrap.ts";

const enum StreamBaseStateFields {
  kReadBytesOrError,
  kArrayBufferOffset,
  kBytesWritten,
  kLastWriteWasAsync,
  kNumStreamBaseStateFields,
}

export const kReadBytesOrError = StreamBaseStateFields.kReadBytesOrError;
export const kArrayBufferOffset = StreamBaseStateFields.kArrayBufferOffset;
export const kBytesWritten = StreamBaseStateFields.kBytesWritten;
export const kLastWriteWasAsync = StreamBaseStateFields.kLastWriteWasAsync;
export const kNumStreamBaseStateFields =
  StreamBaseStateFields.kNumStreamBaseStateFields;

export const streamBaseState = new Int32Array(5);

export class WriteWrap<H extends HandleWrap> extends AsyncWrap {
  handle!: H;
  oncomplete!: (status: number) => void;
  async!: boolean;
  bytes!: number;
  buffer!: unknown;
  callback!: unknown;
  _chunks!: unknown[];

  constructor() {
    super(providerType.WRITEWRAP);
  }
}

export class ShutdownWrap<H extends HandleWrap> extends AsyncWrap {
  handle!: H;
  oncomplete!: (status: number) => void;
  callback!: () => void;

  constructor() {
    super(providerType.SHUTDOWNWRAP);
  }
}
