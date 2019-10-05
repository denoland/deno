// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
/* eslint-disable @typescript-eslint/no-explicit-any */
import * as dispatch from "./dispatch.ts";
import { sendAsync, sendSync } from "./dispatch_json.ts";
import { log } from "./util.ts";
import { TextDecoder, TextEncoder } from "./text_encoding.ts";
import { window } from "./window.ts";
import { blobURLMap } from "./url.ts";
import { blobBytesWeakMap } from "./blob.ts";

const encoder = new TextEncoder();
const decoder = new TextDecoder();

export function encodeMessage(data: any): Uint8Array {
  const dataJson = JSON.stringify(data);
  return encoder.encode(dataJson);
}

export function decodeMessage(dataIntArray: Uint8Array): any {
  const dataJson = decoder.decode(dataIntArray);
  return JSON.parse(dataJson);
}

function createWorker(
  specifier: string,
  includeDenoNamespace: boolean,
  hasSourceCode: boolean,
  sourceCode: Uint8Array
): number {
  return sendSync(dispatch.OP_CREATE_WORKER, {
    specifier,
    includeDenoNamespace,
    hasSourceCode,
    sourceCode: new TextDecoder().decode(sourceCode)
  });
}

async function hostGetWorkerClosed(rid: number): Promise<void> {
  await sendAsync(dispatch.OP_HOST_GET_WORKER_CLOSED, { rid });
}

function hostPostMessage(rid: number, data: any): void {
  const dataIntArray = encodeMessage(data);
  sendSync(dispatch.OP_HOST_POST_MESSAGE, { rid }, dataIntArray);
}

async function hostGetMessage(rid: number): Promise<any> {
  const res = await sendAsync(dispatch.OP_HOST_GET_MESSAGE, { rid });

  if (res.data != null) {
    return decodeMessage(new Uint8Array(res.data));
  } else {
    return null;
  }
}

// Stuff for workers
export const onmessage: (e: { data: any }) => void = (): void => {};

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

export async function workerMain(): Promise<void> {
  log("workerMain");

  while (!isClosing) {
    const data = await getMessage();
    if (data == null) {
      log("workerMain got null message. quitting.");
      break;
    }

    if (window["onmessage"]) {
      const event = { data };
      const result: void | Promise<void> = window.onmessage(event);
      if (result && "then" in result) {
        await result;
      }
    }

    if (!window["onmessage"]) {
      break;
    }
  }
}

export interface Worker {
  onerror?: () => void;
  onmessage?: (e: { data: any }) => void;
  onmessageerror?: () => void;
  postMessage(data: any): void;
  closed: Promise<void>;
}

// TODO(kevinkassimo): Maybe implement reasonable web worker options?
// eslint-disable-next-line @typescript-eslint/no-empty-interface
export interface WorkerOptions {}

/** Extended Deno Worker initialization options.
 * `noDenoNamespace` hides global `window.Deno` namespace for
 * spawned worker and nested workers spawned by it (default: false).
 */
export interface DenoWorkerOptions extends WorkerOptions {
  noDenoNamespace?: boolean;
}

export class WorkerImpl implements Worker {
  private readonly rid: number;
  private isClosing = false;
  private readonly isClosedPromise: Promise<void>;
  public onerror?: () => void;
  public onmessage?: (data: any) => void;
  public onmessageerror?: () => void;

  constructor(specifier: string, options?: DenoWorkerOptions) {
    let hasSourceCode = false;
    let sourceCode = new Uint8Array();

    let includeDenoNamespace = true;
    if (options && options.noDenoNamespace) {
      includeDenoNamespace = false;
    }
    // Handle blob URL.
    if (specifier.startsWith("blob:")) {
      hasSourceCode = true;
      const b = blobURLMap.get(specifier);
      if (!b) {
        throw new Error("No Blob associated with the given URL is found");
      }
      const blobBytes = blobBytesWeakMap.get(b!);
      if (!blobBytes) {
        throw new Error("Invalid Blob");
      }
      sourceCode = blobBytes!;
    }

    this.rid = createWorker(
      specifier,
      includeDenoNamespace,
      hasSourceCode,
      sourceCode
    );
    this.run();
    this.isClosedPromise = hostGetWorkerClosed(this.rid);
    this.isClosedPromise.then(
      (): void => {
        this.isClosing = true;
      }
    );
  }

  get closed(): Promise<void> {
    return this.isClosedPromise;
  }

  postMessage(data: any): void {
    hostPostMessage(this.rid, data);
  }

  private async run(): Promise<void> {
    while (!this.isClosing) {
      const data = await hostGetMessage(this.rid);
      if (data == null) {
        log("worker got null message. quitting.");
        break;
      }
      // TODO(afinch7) stop this from eating messages before onmessage has been assigned
      if (this.onmessage) {
        const event = { data };
        this.onmessage(event);
      }
    }
  }
}
