// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
/* eslint-disable @typescript-eslint/no-explicit-any */
import { sendAsync, sendSync } from "./ops/dispatch_json.ts";
import { log } from "./util.ts";
import { TextDecoder, TextEncoder } from "./web/text_encoding.ts";
/*
import { blobURLMap } from "./web/url.ts";
import { blobBytesWeakMap } from "./web/blob.ts";
*/
import { Event } from "./web/event.ts";
import { EventTarget } from "./web/event_target.ts";

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
): { id: number } {
  return sendSync("op_create_worker", {
    specifier,
    hasSourceCode,
    sourceCode: new TextDecoder().decode(sourceCode),
    name
  });
}

function hostTerminateWorker(id: number): void {
  sendSync("op_host_terminate_worker", { id });
}

function hostPostMessage(id: number, data: any): void {
  const dataIntArray = encodeMessage(data);
  sendSync("op_host_post_message", { id }, dataIntArray);
}

interface WorkerEvent {
  event: "error" | "msg" | "close";
  data?: any;
  error?: any;
}

async function hostGetMessage(id: number): Promise<any> {
  return await sendAsync("op_host_get_message", { id });
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
  public onerror?: (e: any) => void;
  public onmessage?: (data: any) => void;
  public onmessageerror?: () => void;
  private name: string;
  private terminated = false;

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

    this.name = options?.name ?? "unknown";
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

    const { id } = createWorker(
      specifier,
      hasSourceCode,
      sourceCode,
      options?.name
    );
    this.id = id;
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
    while (!this.terminated) {
      const event = await hostGetMessage(this.id);

      // If terminate was called then we ignore all messages
      if (this.terminated) {
        return;
      }

      const type = event.type;

      if (type === "msg") {
        if (this.onmessage) {
          const message = decodeMessage(new Uint8Array(event.data));
          this.onmessage({ data: message });
        }
        continue;
      }

      if (type === "error") {
        if (!this.handleError(event.error)) {
          throw Error(event.error.message);
        }
        continue;
      }

      if (type === "close") {
        log(`Host got "close" message from worker: ${this.name}`);
        this.terminated = true;
        return;
      }

      throw new Error(`Unknown worker event: "${type}"`);
    }
  }

  postMessage(data: any): void {
    if (this.terminated) {
      return;
    }

    hostPostMessage(this.id, data);
  }

  terminate(): void {
    if (!this.terminated) {
      this.terminated = true;
      hostTerminateWorker(this.id);
    }
  }
}
