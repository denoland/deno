// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
/* eslint-disable @typescript-eslint/no-explicit-any */
import * as dispatch from "./dispatch.ts";
import { sendAsync, sendSync } from "./dispatch_json.ts";
import { log } from "./util.ts";
import { TextDecoder, TextEncoder } from "./text_encoding.ts";
import { initOps } from "./os.ts";

const encoder = new TextEncoder();
const decoder = new TextDecoder();

function encodeMessage(data: any): Uint8Array {
  const dataJson = JSON.stringify(data);
  return encoder.encode(dataJson);
}

function decodeMessage(dataIntArray: Uint8Array): any {
  const dataJson = decoder.decode(dataIntArray);
  return JSON.parse(dataJson);
}

// Stuff for workers
export const onmessage: (e: { data: any }) => void = (): void => {};
export const onerror: (e: { data: any }) => void = (): void => {};

export function postMessage(data: any): void {
  const dataIntArray = encodeMessage(data);
  sendSync(dispatch.OP_WORKER_POST_MESSAGE, {}, dataIntArray);
}

export async function getMessage(): Promise<any> {
  log("getMessage");
  const res = await sendAsync(dispatch.OP_WORKER_GET_MESSAGE);
  if (res.data != null) {
    return decodeMessage(new Uint8Array(res.data));
  } else {
    return null;
  }
}

export let isClosing = false;

export function workerClose(): void {
  isClosing = true;
}

export async function bootstrapWorkerRuntime(): Promise<void> {
  initOps();

  log("bootstrapWorkerRuntime");

  while (!isClosing) {
    const data = await getMessage();
    if (data == null) {
      log("bootstrapWorkerRuntime got null message. quitting.");
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
