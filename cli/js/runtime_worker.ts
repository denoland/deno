// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// This module is the entry point for "worker" isolate, ie. the one
// that is created using `new Worker()` JS API.
//
// It provides two functions that should be called by Rust:
//  - `bootstrapWorkerRuntime` - must be called once, when Isolate is created.
//   It sets up runtime by providing globals for `DedicatedWorkerScope`.
//  - `runWorkerMessageLoop` - starts receiving messages from parent worker,
//   can be called multiple times - eg. to restart worker execution after
//   exception occurred and was handled by parent worker

/* eslint-disable @typescript-eslint/no-explicit-any */
import {
  readOnly,
  writable,
  nonEnumerable,
  windowOrWorkerGlobalScopeMethods,
  windowOrWorkerGlobalScopeProperties,
  eventTargetProperties
} from "./globals.ts";
import * as dispatch from "./dispatch.ts";
import { sendAsync, sendSync } from "./dispatch_json.ts";
import { log } from "./util.ts";
import { TextDecoder, TextEncoder } from "./text_encoding.ts";
import * as runtime from "./runtime.ts";

const encoder = new TextEncoder();
const decoder = new TextDecoder();

// TODO(bartlomieju): remove these funtions
// Stuff for workers
export const onmessage: (e: { data: any }) => void = (): void => {};
export const onerror: (e: { data: any }) => void = (): void => {};

export function postMessage(data: any): void {
  const dataJson = JSON.stringify(data);
  const dataIntArray = encoder.encode(dataJson);
  sendSync(dispatch.OP_WORKER_POST_MESSAGE, {}, dataIntArray);
}

export async function getMessage(): Promise<any> {
  log("getMessage");
  const res = await sendAsync(dispatch.OP_WORKER_GET_MESSAGE);
  if (res.data != null) {
    const dataIntArray = new Uint8Array(res.data);
    const dataJson = decoder.decode(dataIntArray);
    return JSON.parse(dataJson);
  } else {
    return null;
  }
}

let isClosing = false;
let hasBootstrapped = false;

export function close(): void {
  isClosing = true;
}

export async function runWorkerMessageLoop(): Promise<void> {
  while (!isClosing) {
    const data = await getMessage();
    if (data == null) {
      log("runWorkerMessageLoop got null message. quitting.");
      break;
    }

    let result: void | Promise<void>;
    const event = { data };

    try {
      if (!globalThis["onmessage"]) {
        break;
      }
      result = globalThis.onmessage!(event);
      if (result && "then" in result) {
        await result;
      }
      if (!globalThis["onmessage"]) {
        break;
      }
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
          continue;
        }
      }
      throw e;
    }
  }
}

export const workerRuntimeGlobalProperties = {
  self: readOnly(globalThis),
  onmessage: writable(onmessage),
  onerror: writable(onerror),
  close: nonEnumerable(close),
  postMessage: writable(postMessage)
};

/**
 * Main method to initialize worker runtime.
 *
 * It sets up global variables for DedicatedWorkerScope,
 * and initializes ops.
 */
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
  runtime.start(false, name);
}
