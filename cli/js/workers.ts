// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
/* eslint-disable @typescript-eslint/no-explicit-any */
import * as dispatch from "./dispatch.ts";
import { sendAsync, sendSync } from "./dispatch_json.ts";
import { log, createResolvable, Resolvable } from "./util.ts";
import { TextDecoder, TextEncoder } from "./text_encoding.ts";
import { window } from "./window.ts";
import { blobURLMap } from "./url.ts";
import { blobBytesWeakMap } from "./blob.ts";
import { EventTarget } from "./event_target.ts";

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
): { id: number; loaded: boolean } {
  return sendSync(dispatch.OP_CREATE_WORKER, {
    specifier,
    includeDenoNamespace,
    hasSourceCode,
    sourceCode: new TextDecoder().decode(sourceCode)
  });
}

async function hostGetWorkerLoaded(id: number): Promise<any> {
  return await sendAsync(dispatch.OP_HOST_GET_WORKER_LOADED, { id });
}

async function hostPollWorker(id: number): Promise<any> {
  return await sendAsync(dispatch.OP_HOST_POLL_WORKER, { id });
}

function hostCloseWorker(id: number): void {
  sendSync(dispatch.OP_HOST_CLOSE_WORKER, { id });
}

function hostResumeWorker(id: number): void {
  sendSync(dispatch.OP_HOST_RESUME_WORKER, { id });
}

function hostPostMessage(id: number, data: any): void {
  const dataIntArray = encodeMessage(data);
  sendSync(dispatch.OP_HOST_POST_MESSAGE, { id }, dataIntArray);
}

async function hostGetMessage(id: number): Promise<any> {
  const res = await sendAsync(dispatch.OP_HOST_GET_MESSAGE, { id });

  if (res.data != null) {
    return decodeMessage(new Uint8Array(res.data));
  } else {
    return null;
  }
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

export async function workerMain(): Promise<void> {
  log("workerMain");

  while (!isClosing) {
    const data = await getMessage();
    if (data == null) {
      log("workerMain got null message. quitting.");
      break;
    }

    let result: void | Promise<void>;
    const event = { data };

    try {
      result = window.onmessage(event);
      if (result && "then" in result) {
        await result;
      }
      if (!window["onmessage"]) {
        break;
      }
    } catch (e) {
      if (window["onerror"]) {
        const result = window.onerror(
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

export interface Worker {
  onerror?: (e: any) => void;
  onmessage?: (e: { data: any }) => void;
  onmessageerror?: () => void;
  postMessage(data: any): void;
  // TODO(bartlomieju): remove this
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

export class WorkerImpl extends EventTarget implements Worker {
  private readonly id: number;
  private isClosing = false;
  private messageBuffer: any[] = [];
  private ready = false;
  private readonly isClosedPromise: Resolvable<void>;
  public onerror?: (e: any) => void;
  public onmessage?: (data: any) => void;
  public onmessageerror?: () => void;

  constructor(specifier: string, options?: DenoWorkerOptions) {
    super();
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

    const { id, loaded } = createWorker(
      specifier,
      includeDenoNamespace,
      hasSourceCode,
      sourceCode
    );
    this.id = id;
    this.ready = loaded;
    this.isClosedPromise = createResolvable();
    this.poll();
  }

  get closed(): Promise<void> {
    return this.isClosedPromise;
  }

  private handleError(e: any): boolean {
    const event = new window.Event("error", { cancelable: true });
    event.message = e.message;
    event.lineNumber = e.lineNumber ? e.lineNumber + 1 : null;
    event.columnNumber = e.columnNumber ? e.columnNumber + 1 : null;
    event.fileName = e.fileName;
    event.error = null;

    let handled = false;
    if (this.onerror) {
      this.onerror(event);
      if (event.defaultPrevented) {
        handled = true;
      }
    }

    return handled;
  }

  async poll(): Promise<void> {
    // If worker has not been immediately executed
    // then let's await it's readiness
    if (!this.ready) {
      const result = await hostGetWorkerLoaded(this.id);

      if (result.error) {
        if (!this.handleError(result.error)) {
          throw new Error(result.error.message);
        }
        return;
      }
    }

    // drain messages
    for (const data of this.messageBuffer) {
      hostPostMessage(this.id, data);
    }
    this.messageBuffer = [];
    this.ready = true;
    this.run();

    while (true) {
      const result = await hostPollWorker(this.id);

      if (result.error) {
        if (!this.handleError(result.error)) {
          throw Error(result.error.message);
        } else {
          hostResumeWorker(this.id);
        }
      } else {
        this.isClosing = true;
        hostCloseWorker(this.id);
        this.isClosedPromise.resolve();
        break;
      }
    }
  }

  postMessage(data: any): void {
    if (!this.ready) {
      this.messageBuffer.push(data);
      return;
    }

    hostPostMessage(this.id, data);
  }

  private async run(): Promise<void> {
    while (!this.isClosing) {
      const data = await hostGetMessage(this.id);
      if (data == null) {
        log("worker got null message. quitting.");
        break;
      }
      if (this.onmessage) {
        const event = { data };
        this.onmessage(event);
      }
    }
  }
}
