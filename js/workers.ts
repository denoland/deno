// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
/* eslint-disable @typescript-eslint/no-explicit-any */
import { sendAsync, sendSync } from "./dispatch";
import * as msg from "gen/cli/msg_generated";
import * as flatbuffers from "./flatbuffers";
import { assert, log } from "./util";
import { TextDecoder, TextEncoder } from "./text_encoding";
import { window } from "./window";

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

function createWorker(specifier: string): number {
  const builder = flatbuffers.createBuilder();
  const specifier_ = builder.createString(specifier);
  const inner = msg.CreateWorker.createCreateWorker(builder, specifier_);
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

export interface ErrorEvent {
  message: string;
  filename: string;
  lineno: number;
  colno: number;
}

export interface MessageEvent {
  data: any;
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
      try {
        window.onmessage(event);
      } catch (e) {
        const errorEvent: ErrorEvent = {
          message: e.message,
          filename: e.filename,
          lineno: e.lineno,
          colno: e.colno
        };
        postMessage(errorEvent);
        console.error(e);
      }
    }

    if (!window["onmessage"]) {
      break;
    }
  }
}

export interface Worker {
  onerror?: (e: ErrorEvent) => void;
  onmessage?: (e: MessageEvent) => void;
  onmessageerror?: (e: MessageEvent) => void;
  postMessage(data: any): void;
  closed: Promise<void>;
}

export class WorkerImpl implements Worker {
  private readonly rid: number;
  private isClosing: boolean = false;
  private readonly isClosedPromise: Promise<void>;
  public onerror?: (e: ErrorEvent) => void;
  public onmessage?: (e: MessageEvent) => void;
  public onmessageerror?: (e: MessageEvent) => void;

  constructor(specifier: string) {
    this.rid = createWorker(specifier);
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
      // TODO: handle different types of messages
      const msg = await hostGetMessage(this.rid);
      if (msg == null) {
        log("worker got null message. quitting.");
        break;
      }

      if (msg.message) {
        if (this.onerror) {
          this.onerror(msg as ErrorEvent);
        }
        continue;
      }

      // TODO(afinch7) stop this from eating messages before onmessage has been assigned
      if (this.onmessage) {
        this.onmessage(msg as MessageEvent);
      }
    }
  }
}
