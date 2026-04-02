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
// - https://github.com/nodejs/node/blob/master/src/async_wrap-inl.h
// - https://github.com/nodejs/node/blob/master/src/async_wrap.cc
// - https://github.com/nodejs/node/blob/master/src/async_wrap.h

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import { AsyncWrap, op_node_new_async_id } from "ext:core/ops";

export function registerDestroyHook(
  // deno-lint-ignore no-explicit-any
  _target: any,
  _asyncId: number,
  _prop: { destroyed: boolean },
) {
  // TODO(kt3k): implement actual procedures
}

export const constants = { kInit: 0, kBefore: 1, kAfter: 2, kDestroy: 3, kPromiseResolve: 4, kTotals: 5, kCheck: 6, kExecutionAsyncId: 7, kTriggerAsyncId: 8, kAsyncIdCounter: 9, kDefaultTriggerAsyncId: 10, kUsesExecutionAsyncResource: 11, kStackLength: 12 } as const;
export type constants = typeof constants[keyof typeof constants];

const asyncHookFields = new Uint32Array(Object.keys(constants).length);

export { asyncHookFields as async_hook_fields };

// Increment the internal id counter and return the value.
export function newAsyncId() {
  return op_node_new_async_id();
}

export const UidFields = { kExecutionAsyncId: 0, kTriggerAsyncId: 1, kDefaultTriggerAsyncId: 2, kUidFieldsCount: 3 } as const;
export type UidFields = typeof UidFields[keyof typeof UidFields];

const asyncIdFields = new Float64Array(Object.keys(UidFields).length);

// `kDefaultTriggerAsyncId` should be `-1`, this indicates that there is no
// specified default value and it should fallback to the executionAsyncId.
// 0 is not used as the magic value, because that indicates a missing
// context which is different from a default context.
asyncIdFields[UidFields.kDefaultTriggerAsyncId] = -1;

export { asyncIdFields };

export const providerType = { NONE: 0, DIRHANDLE: 1, DNSCHANNEL: 2, ELDHISTOGRAM: 3, FILEHANDLE: 4, FILEHANDLECLOSEREQ: 5, FIXEDSIZEBLOBCOPY: 6, FSEVENTWRAP: 7, FSREQCALLBACK: 8, FSREQPROMISE: 9, GETADDRINFOREQWRAP: 10, GETNAMEINFOREQWRAP: 11, HEAPSNAPSHOT: 12, HTTP2SESSION: 13, HTTP2STREAM: 14, HTTP2PING: 15, HTTP2SETTINGS: 16, HTTPINCOMINGMESSAGE: 17, HTTPCLIENTREQUEST: 18, JSSTREAM: 19, JSUDPWRAP: 20, MESSAGEPORT: 21, PIPECONNECTWRAP: 22, PIPESERVERWRAP: 23, PIPEWRAP: 24, PROCESSWRAP: 25, PROMISE: 26, QUERYWRAP: 27, SHUTDOWNWRAP: 28, SIGNALWRAP: 29, STATWATCHER: 30, STREAMPIPE: 31, TCPCONNECTWRAP: 32, TCPSERVERWRAP: 33, TCPWRAP: 34, TTYWRAP: 35, UDPSENDWRAP: 36, UDPWRAP: 37, SIGINTWATCHDOG: 38, WORKER: 39, WORKERHEAPSNAPSHOT: 40, WRITEWRAP: 41, ZLIB: 42 } as const;
export type providerType = typeof providerType[keyof typeof providerType];

export { AsyncWrap };
