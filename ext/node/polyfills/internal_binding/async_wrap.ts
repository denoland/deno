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

(function () {
const { core, primordials } = __bootstrap;
const { AsyncWrap, op_node_new_async_id } = core.ops;
const {
  ArrayPrototypePush,
  ArrayPrototypeSplice,
  Float64Array,
  ObjectKeys,
  Uint32Array,
} = primordials;

function registerDestroyHook(
  // deno-lint-ignore no-explicit-any
  _target: any,
  _asyncId: number,
  _prop: { destroyed: boolean },
) {
  // TODO(kt3k): implement actual procedures
}

// ---------------------------------------------------------------------------
// queueDestroyAsyncId
//
// Mirrors Node's `async_wrap.queueDestroyAsyncId` (src/async_wrap.cc). Async
// ids passed here are batched and their `destroy` hooks are fired on the next
// turn of the event loop via a single `setImmediate`, matching Node's
// `DestroyAsyncIdsCallback`. The actual hook emission lives in
// `internal/async_hooks.ts` (where the active hook list is), which registers
// its `emitDestroy` via `setDestroyCallback()`.
// ---------------------------------------------------------------------------

let destroyCallback: ((asyncId: number) => void) | null = null;
const destroyAsyncIdList: number[] = [];
let destroyScheduled = false;
// deno-lint-ignore no-explicit-any
let lazyTimers: (() => any) | null = null;

function setDestroyCallback(cb: (asyncId: number) => void) {
  destroyCallback = cb;
}

function processDestroyAsyncIds() {
  destroyScheduled = false;
  // Process a snapshot: any ids queued while emitting destroy hooks below will
  // schedule a fresh immediate of their own.
  const ids = ArrayPrototypeSplice(destroyAsyncIdList, 0);
  if (destroyCallback === null) {
    return;
  }
  for (let i = 0; i < ids.length; i++) {
    destroyCallback(ids[i]);
  }
}

function queueDestroyAsyncId(asyncId: number) {
  ArrayPrototypePush(destroyAsyncIdList, asyncId);
  if (!destroyScheduled) {
    destroyScheduled = true;
    if (lazyTimers === null) {
      lazyTimers = core.createLazyLoader("node:timers");
    }
    lazyTimers().setImmediate(processDestroyAsyncIds);
  }
}

enum constants {
  kInit,
  kBefore,
  kAfter,
  kDestroy,
  kPromiseResolve,
  kTotals,
  kCheck,
  kExecutionAsyncId,
  kTriggerAsyncId,
  kAsyncIdCounter,
  kDefaultTriggerAsyncId,
  kUsesExecutionAsyncResource,
  kStackLength,
}

const asyncHookFields = new Uint32Array(ObjectKeys(constants).length);

// Increment the internal id counter and return the value.
function newAsyncId() {
  return op_node_new_async_id();
}

enum UidFields {
  kExecutionAsyncId,
  kTriggerAsyncId,
  kDefaultTriggerAsyncId,
  kUidFieldsCount,
}

const asyncIdFields = new Float64Array(ObjectKeys(UidFields).length);

// `kDefaultTriggerAsyncId` should be `-1`, this indicates that there is no
// specified default value and it should fallback to the executionAsyncId.
// 0 is not used as the magic value, because that indicates a missing
// context which is different from a default context.
asyncIdFields[UidFields.kDefaultTriggerAsyncId] = -1;

enum providerType {
  NONE,
  DIRHANDLE,
  DNSCHANNEL,
  ELDHISTOGRAM,
  FILEHANDLE,
  FILEHANDLECLOSEREQ,
  FIXEDSIZEBLOBCOPY,
  FSEVENTWRAP,
  FSREQCALLBACK,
  FSREQPROMISE,
  GETADDRINFOREQWRAP,
  GETNAMEINFOREQWRAP,
  HEAPSNAPSHOT,
  HTTP2SESSION,
  HTTP2STREAM,
  HTTP2PING,
  HTTP2SETTINGS,
  HTTPINCOMINGMESSAGE,
  HTTPCLIENTREQUEST,
  JSSTREAM,
  JSUDPWRAP,
  MESSAGEPORT,
  PIPECONNECTWRAP,
  PIPESERVERWRAP,
  PIPEWRAP,
  PROCESSWRAP,
  PROMISE,
  QUERYWRAP,
  SHUTDOWNWRAP,
  SIGNALWRAP,
  STATWATCHER,
  STREAMPIPE,
  TCPCONNECTWRAP,
  TCPSERVERWRAP,
  TCPWRAP,
  TTYWRAP,
  UDPSENDWRAP,
  UDPWRAP,
  SIGINTWATCHDOG,
  WORKER,
  WORKERHEAPSNAPSHOT,
  WRITEWRAP,
  ZLIB,
}

return {
  async_hook_fields: asyncHookFields,
  asyncIdFields,
  AsyncWrap,
  registerDestroyHook,
  newAsyncId,
  constants,
  UidFields,
  providerType,
  queueDestroyAsyncId,
  setDestroyCallback,
};
})();
