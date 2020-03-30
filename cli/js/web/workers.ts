// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
/* eslint-disable @typescript-eslint/no-explicit-any */
import {
  createWorker,
  hostTerminateWorker,
  hostPostMessage,
  hostGetMessage,
} from "../ops/worker_host.ts";
import { log } from "../util.ts";
import { TextDecoder, TextEncoder } from "./text_encoding.ts";
/*
import { blobURLMap } from "./web/url.ts";
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

interface WorkerEvent {
  event: "error" | "msg" | "close";
  data?: any;
  error?: any;
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
  readonly #id: number;
  #name: string;
  #terminated = false;

  public onerror?: (e: any) => void;
  public onmessage?: (data: any) => void;
  public onmessageerror?: () => void;

  constructor(specifier: string, options?: WorkerOptions) {
    super();
    const { type = "classic", name = "unknown" } = options ?? {};

    if (type !== "module") {
      throw new Error(
        'Not yet implemented: only "module" type workers are supported'
      );
    }

    this.#name = name;
    const hasSourceCode = false;
    const sourceCode = decoder.decode(new Uint8Array());

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
    this.#id = id;
    this.poll();
  }

  #handleError = (e: any): boolean => {
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
  };

  async poll(): Promise<void> {
    while (!this.#terminated) {
      const event = await hostGetMessage(this.#id);

      // If terminate was called then we ignore all messages
      if (this.#terminated) {
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
        if (!this.#handleError(event.error)) {
          throw Error(event.error.message);
        }
        continue;
      }

      if (type === "close") {
        log(`Host got "close" message from worker: ${this.#name}`);
        this.#terminated = true;
        return;
      }

      throw new Error(`Unknown worker event: "${type}"`);
    }
  }

  postMessage(data: any): void {
    if (this.#terminated) {
      return;
    }

    hostPostMessage(this.#id, encodeMessage(data));
  }

  terminate(): void {
    if (!this.#terminated) {
      this.#terminated = true;
      hostTerminateWorker(this.#id);
    }
  }
}
