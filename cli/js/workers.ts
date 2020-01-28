// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
/* eslint-disable @typescript-eslint/no-explicit-any */
import * as dispatch from "./dispatch.ts";
import { sendAsync, sendSync } from "./dispatch_json.ts";
import { log } from "./util.ts";
import { TextDecoder, TextEncoder } from "./text_encoding.ts";
/*
import { blobURLMap } from "./url.ts";
import { blobBytesWeakMap } from "./blob.ts";
*/
import { Event } from "./event.ts";
import { EventTarget } from "./event_target.ts";

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

function createWorker(
  specifier: string,
  hasSourceCode: boolean,
  sourceCode: Uint8Array,
  name?: string
): { id: number; loaded: boolean } {
  return sendSync(dispatch.OP_CREATE_WORKER, {
    specifier,
    hasSourceCode,
    sourceCode: new TextDecoder().decode(sourceCode),
    name
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

export interface Worker {
  onerror?: (e: any) => void;
  onmessage?: (e: { data: any }) => void;
  onmessageerror?: () => void;
  postMessage(data: any): void;
  terminate(): void;
}

export interface WorkerOptions {
  type?: "classic" | "module";
  name?: string;
}

export class WorkerImpl extends EventTarget implements Worker {
  private readonly id: number;
  private isClosing = false;
  private messageBuffer: any[] = [];
  private ready = false;
  public onerror?: (e: any) => void;
  public onmessage?: (data: any) => void;
  public onmessageerror?: () => void;

  constructor(specifier: string, options?: WorkerOptions) {
    super();

    let type = "classic";

    if (options?.type) {
      type = options.type;
    }

    if (type !== "module") {
      throw new Error(
        'Not yet implemented: only "module" type workers are supported'
      );
    }

    const hasSourceCode = false;
    const sourceCode = new Uint8Array();

    /* TODO(bartlomieju):
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
    */

    const { id, loaded } = createWorker(
      specifier,
      hasSourceCode,
      sourceCode,
      options?.name
    );
    this.id = id;
    this.ready = loaded;
    this.poll();
  }

  private handleError(e: any): boolean {
    // TODO: this is being handled in a type unsafe way, it should be type safe
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const event = new Event("error", { cancelable: true }) as any;
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

  terminate(): void {
    throw new Error("Not yet implemented");
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
