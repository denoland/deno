// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// This module is the entry point for "worker" isolate, ie. the one
// that is created using `new Worker()` JS API.
//
// It provides a single function that should be called by Rust:
//  - `bootstrapWorkerRuntime` - must be called once, when Isolate is created.
//   It sets up runtime by providing globals for `DedicatedWorkerScope`.

/* eslint-disable @typescript-eslint/no-explicit-any */
import {
  readOnly,
  writable,
  nonEnumerable,
  windowOrWorkerGlobalScopeMethods,
  windowOrWorkerGlobalScopeProperties,
  eventTargetProperties,
} from "./globals.ts";
import * as webWorkerOps from "./ops/web_worker.ts";
import { LocationImpl } from "./web/location.ts";
import { log, assert, immutableDefine } from "./util.ts";
import { TextEncoder } from "./web/text_encoding.ts";
import * as runtime from "./runtime.ts";

const encoder = new TextEncoder();

// TODO(bartlomieju): remove these funtions
// Stuff for workers
export const onmessage: (e: { data: any }) => void = (): void => {};
export const onerror: (e: { data: any }) => void = (): void => {};

export function postMessage(data: any): void {
  const dataJson = JSON.stringify(data);
  const dataIntArray = encoder.encode(dataJson);
  webWorkerOps.postMessage(dataIntArray);
}

let isClosing = false;
let hasBootstrapped = false;

export function close(): void {
  if (isClosing) {
    return;
  }

  isClosing = true;
  webWorkerOps.close();
}

export async function workerMessageRecvCallback(data: string): Promise<void> {
  let result: void | Promise<void>;
  const event = { data };

  try {
    //
    if (globalThis["onmessage"]) {
      result = globalThis.onmessage!(event);
      if (result && "then" in result) {
        await result;
      }
    }

    // TODO: run the rest of liteners
  } catch (e) {
    if (globalThis["onerror"]) {
      const result = globalThis.onerror(
        e.message,
        e.fileName,
        e.lineNumber,
        e.columnNumber,
        e
      );
      if (result === true) {
        return;
      }
    }
    throw e;
  }
}

export const workerRuntimeGlobalProperties = {
  self: readOnly(globalThis),
  onmessage: writable(onmessage),
  onerror: writable(onerror),
  // TODO: should be readonly?
  close: nonEnumerable(close),
  postMessage: writable(postMessage),
  workerMessageRecvCallback: nonEnumerable(workerMessageRecvCallback),
};

export function bootstrapWorkerRuntime(name: string): void {
  if (hasBootstrapped) {
    throw new Error("Worker runtime already bootstrapped");
  }
  log("bootstrapWorkerRuntime");
  hasBootstrapped = true;
  Object.defineProperties(globalThis, windowOrWorkerGlobalScopeMethods);
  Object.defineProperties(globalThis, windowOrWorkerGlobalScopeProperties);
  Object.defineProperties(globalThis, workerRuntimeGlobalProperties);
  Object.defineProperties(globalThis, eventTargetProperties);
  Object.defineProperties(globalThis, { name: readOnly(name) });
  const s = runtime.start(name);

  const location = new LocationImpl(s.location);
  immutableDefine(globalThis, "location", location);
  Object.freeze(globalThis.location);

  // globalThis.Deno is not available in worker scope
  delete globalThis.Deno;
  assert(globalThis.Deno === undefined);
}
