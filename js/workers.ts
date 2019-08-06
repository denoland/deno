// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
/* eslint-disable @typescript-eslint/no-explicit-any */
import { sendAsync, sendSync } from "./dispatch";
import * as msg from "gen/cli/msg_generated";
import * as flatbuffers from "./flatbuffers";
import { assert, log } from "./util";
import { TextDecoder, TextEncoder } from "./text_encoding";
import { window } from "./window";
import { blobURLMap } from "./url";
import { blobBytesWeakMap } from "./blob";

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
  const builder = flatbuffers.createBuilder();
  const specifier_ = builder.createString(specifier);
  const sourceCode_ = builder.createString(sourceCode);
  const inner = msg.CreateWorker.createCreateWorker(
    builder,
    specifier_,
    includeDenoNamespace,
    hasSourceCode,
    sourceCode_
  );
  const baseRes = sendSync(builder, msg.Any.CreateWorker, inner);
  assert(baseRes != null);
  assert(
    msg.Any.CreateWorkerRes === baseRes!.innerType(),
    `base.innerType() unexpectedly is ${baseRes!.innerType()}`
  );
  const res = new msg.CreateWorkerRes();
  assert(baseRes!.inner(res) != null);
  return res.rid();
}

async function hostGetWorkerClosed(rid: number): Promise<void> {
  const builder = flatbuffers.createBuilder();
  const inner = msg.HostGetWorkerClosed.createHostGetWorkerClosed(builder, rid);
  await sendAsync(builder, msg.Any.HostGetWorkerClosed, inner);
}

function hostPostMessage(rid: number, data: any): void {
  const dataIntArray = encodeMessage(data);
  const builder = flatbuffers.createBuilder();
  const inner = msg.HostPostMessage.createHostPostMessage(builder, rid);
  const baseRes = sendSync(
    builder,
    msg.Any.HostPostMessage,
    inner,
    dataIntArray
  );
  assert(baseRes != null);
}

async function hostGetMessage(rid: number): Promise<any> {
  const builder = flatbuffers.createBuilder();
  const inner = msg.HostGetMessage.createHostGetMessage(builder, rid);
  const baseRes = await sendAsync(builder, msg.Any.HostGetMessage, inner);
  assert(baseRes != null);
  assert(
    msg.Any.HostGetMessageRes === baseRes!.innerType(),
    `base.innerType() unexpectedly is ${baseRes!.innerType()}`
  );
  const res = new msg.HostGetMessageRes();
  assert(baseRes!.inner(res) != null);

  const dataArray = res.dataArray();
  if (dataArray != null) {
    return decodeMessage(dataArray);
  } else {
    return null;
  }
}

// Stuff for workers
export let onmessage: (e: { data: any }) => void = (): void => {};

export function postMessage(data: any): void {
  const dataIntArray = encodeMessage(data);
  const builder = flatbuffers.createBuilder();
  const inner = msg.WorkerPostMessage.createWorkerPostMessage(builder);
  const baseRes = sendSync(
    builder,
    msg.Any.WorkerPostMessage,
    inner,
    dataIntArray
  );
  assert(baseRes != null);
}

export async function getMessage(): Promise<any> {
  log("getMessage");
  const builder = flatbuffers.createBuilder();
  const inner = msg.WorkerGetMessage.createWorkerGetMessage(
    builder,
    0 /* unused */
  );
  const baseRes = await sendAsync(builder, msg.Any.WorkerGetMessage, inner);
  assert(baseRes != null);
  assert(
    msg.Any.WorkerGetMessageRes === baseRes!.innerType(),
    `base.innerType() unexpectedly is ${baseRes!.innerType()}`
  );
  const res = new msg.WorkerGetMessageRes();
  assert(baseRes!.inner(res) != null);

  const dataArray = res.dataArray();
  if (dataArray != null) {
    return decodeMessage(dataArray);
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
      window.onmessage(event);
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
  private isClosing: boolean = false;
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
