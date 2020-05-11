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
import { ErrorEventImpl as ErrorEvent } from "./error_event.ts";
import { EventImpl as Event } from "./event.ts";
import { EventTargetImpl as EventTarget } from "./event_target.ts";

const encoder = new TextEncoder();
const decoder = new TextDecoder();

export interface MessageEventInit extends EventInit {
  data?: any;
  origin?: string;
  lastEventId?: string;
}

export class MessageEvent extends Event {
  readonly data: any;
  readonly origin: string;
  readonly lastEventId: string;

  constructor(type: string, eventInitDict?: MessageEventInit) {
    super(type, {
      bubbles: eventInitDict?.bubbles ?? false,
      cancelable: eventInitDict?.cancelable ?? false,
      composed: eventInitDict?.composed ?? false,
    });

    this.data = eventInitDict?.data ?? null;
    this.origin = eventInitDict?.origin ?? "";
    this.lastEventId = eventInitDict?.lastEventId ?? "";
  }
}

function encodeMessage(data: any): Uint8Array {
  const dataJson = JSON.stringify(data);
  return encoder.encode(dataJson);
}

function decodeMessage(dataIntArray: Uint8Array): any {
  const dataJson = decoder.decode(dataIntArray);
  return JSON.parse(dataJson);
}

interface WorkerHostError {
  message: string;
  fileName?: string;
  lineNumber?: number;
  columnNumber?: number;
}

interface WorkerHostMessage {
  type: "terminalError" | "error" | "msg";
  data?: any;
  error?: WorkerHostError;
}

export interface Worker {
  onerror?: (e: ErrorEvent) => void;
  onmessage?: (e: MessageEvent) => void;
  onmessageerror?: (e: MessageEvent) => void;
  postMessage(data: any): void;
  terminate(): void;
}

export interface WorkerOptions {
  type?: "classic" | "module";
  name?: string;
  deno?: boolean;
}

export class WorkerImpl extends EventTarget implements Worker {
  readonly #id: number;
  #name: string;
  #terminated = false;

  public onerror?: (e: ErrorEvent) => void;
  public onmessage?: (e: MessageEvent) => void;
  public onmessageerror?: (e: MessageEvent) => void;

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

    const useDenoNamespace = options ? !!options.deno : false;

    const { id } = createWorker(
      specifier,
      hasSourceCode,
      sourceCode,
      useDenoNamespace,
      options?.name
    );
    this.#id = id;
    this.#poll();
  }

  #handleMessage = (msgData: any): void => {
    let data;
    try {
      data = decodeMessage(new Uint8Array(msgData));
    } catch (e) {
      const msgErrorEvent = new MessageEvent("messageerror", {
        cancelable: false,
        data,
      });
      if (this.onmessageerror) {
        this.onmessageerror(msgErrorEvent);
      }
      return;
    }

    const msgEvent = new MessageEvent("message", {
      cancelable: false,
      data,
    });

    if (this.onmessage) {
      this.onmessage(msgEvent);
    }

    this.dispatchEvent(msgEvent);
  };

  #handleError = (e: WorkerHostError): boolean => {
    const event = new ErrorEvent("error", {
      cancelable: true,
      message: e.message,
      lineno: e.lineNumber ? e.lineNumber + 1 : undefined,
      colno: e.columnNumber ? e.columnNumber + 1 : undefined,
      filename: e.fileName,
      error: null,
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
  };

  #poll = async (): Promise<void> => {
    while (!this.#terminated) {
      const event = (await hostGetMessage(this.#id)) as WorkerHostMessage;

      // If terminate was called then we ignore all messages
      if (this.#terminated) {
        return;
      }

      const type = event.type;

      if (type === "terminalError") {
        this.#terminated = true;
        if (!this.#handleError(event.error!)) {
          throw Error(event.error!.message);
        }
        continue;
      }

      if (type === "msg") {
        this.#handleMessage(event.data);
        continue;
      }

      if (type === "error") {
        if (!this.#handleError(event.error!)) {
          throw Error(event.error!.message);
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
  };

  postMessage(message: any, transferOrOptions?: any): void {
    if (transferOrOptions) {
      throw new Error(
        "Not yet implemented: `transfer` and `options` are not supported."
      );
    }

    if (this.#terminated) {
      return;
    }

    hostPostMessage(this.#id, encodeMessage(message));
  }

  terminate(): void {
    if (!this.#terminated) {
      this.#terminated = true;
      hostTerminateWorker(this.#id);
    }
  }
}
