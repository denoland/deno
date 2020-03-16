// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
/* eslint-disable @typescript-eslint/no-explicit-any */
import {
  createWorker,
  hostTerminateWorker,
  hostPostMessage,
  hostGetMessage
} from "../ops/worker_host.ts";
import * as domTypes from "./dom_types.ts";
import { log, notImplemented } from "../util.ts";
import { TextDecoder, TextEncoder } from "./text_encoding.ts";
/*
import { blobURLMap } from "./web/url.ts";
import { blobBytesWeakMap } from "./web/blob.ts";
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

export interface PostMessageOptions {
  transfer: object[];
}
export interface MessagePort extends EventTarget {
  postMessage(message: any, transfer: object[]): void;
  postMessage(message: any, options: PostMessageOptions): void;
}
export type MessageEventSource = MessagePort | Worker; // | WindowProxy
export interface MessageEventInit extends domTypes.EventInit {
  data?: any;
  origin?: string;
  lastEventId?: string;
  source?: MessageEventSource | null;
  ports?: MessagePort[];
}
export class MessageEvent extends Event {
  readonly data: any;
  readonly origin: string;
  readonly lastEventId: string;
  readonly source: MessageEventSource | null;
  //readonly ports: MessagePort[];

  constructor(type: string, eventInitDict?: MessageEventInit) {
    super(type, {
      bubbles: eventInitDict.bubbles,
      cancelable: eventInitDict.cancelable,
      composed: eventInitDict.composed
    });

    if (eventInitDict.ports) {
      notImplemented();
    }

    this.data = eventInitDict.data;
    this.origin = eventInitDict.origin;
    this.lastEventId = eventInitDict.lastEventId;
    this.source = eventInitDict.source;
  }
}

export interface ErrorEventInit extends domTypes.EventInit {
  message?: string;
  filename?: string;
  lineno?: number;
  colno?: number;
  error?: any;
}
export class ErrorEvent extends Event {
  readonly message: string;
  readonly filename: string;
  readonly lineno: number;
  readonly colno: number;
  readonly error: any;

  constructor(type: string, eventInitDict?: ErrorEventInit) {
    super(type, {
      bubbles: eventInitDict.bubbles,
      cancelable: eventInitDict.cancelable,
      composed: eventInitDict.composed
    });

    this.message = eventInitDict.message;
    this.filename = eventInitDict.filename;
    this.lineno = eventInitDict.lineno;
    this.colno = eventInitDict.colno;
    this.error = eventInitDict.error;
  }
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
  private readonly id: number;
  private isClosing = false;
  public onerror?: (e: any) => void;
  public onmessage?: (data: any) => void;
  public onmessageerror?: () => void;
  private name: string;
  private terminated = false;

  constructor(specifier: string, options?: WorkerOptions) {
    super();
    const { type = "classic", name = "unknown" } = options ?? {};

    if (type !== "module") {
      throw new Error(
        'Not yet implemented: only "module" type workers are supported'
      );
    }

    this.name = name;
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
    this.id = id;
    this.poll();
  }

  private handleError(e: any): boolean {
    const event = new ErrorEvent("error", {
      cancelable: true,
      message: e.message,
      lineno: e.lineNumber ? e.lineNumber + 1 : null,
      colno: e.columnNumber ? e.columnNumber + 1 : null,
      filename: e.fileName,
      error: null
    });

    let handled = false;
    if (this.onerror) {
      this.onerror(event);
    }

    this.dispatchEvent(event);
    if (event.defaultPrevented) {
      handled = true;
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
        const ev = new MessageEvent("message", {
          cancelable: false,
          data: event.data
        });

        this.dispatchEvent(ev);
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

    hostPostMessage(this.id, encodeMessage(data));
  }

  terminate(): void {
    if (!this.terminated) {
      this.terminated = true;
      hostTerminateWorker(this.id);
    }
  }
}
