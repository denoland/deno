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
  msg.CreateWorker.startCreateWorker(builder);
  msg.CreateWorker.addSpecifier(builder, specifier_);
  const inner = msg.CreateWorker.endCreateWorker(builder);
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
  msg.HostGetWorkerClosed.startHostGetWorkerClosed(builder);
  msg.HostGetWorkerClosed.addRid(builder, rid);
  const inner = msg.HostGetWorkerClosed.endHostGetWorkerClosed(builder);
  await sendAsync(builder, msg.Any.HostGetWorkerClosed, inner);
}

function hostPostMessage(rid: number, data: any): void {
  const dataIntArray = encodeMessage(data);
  const builder = flatbuffers.createBuilder();
  msg.HostPostMessage.startHostPostMessage(builder);
  msg.HostPostMessage.addRid(builder, rid);
  const inner = msg.HostPostMessage.endHostPostMessage(builder);
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
  msg.HostGetMessage.startHostGetMessage(builder);
  msg.HostGetMessage.addRid(builder, rid);
  const inner = msg.HostGetMessage.endHostGetMessage(builder);
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
  msg.WorkerPostMessage.startWorkerPostMessage(builder);
  const inner = msg.WorkerPostMessage.endWorkerPostMessage(builder);
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
  msg.WorkerGetMessage.startWorkerGetMessage(builder);
  const inner = msg.WorkerGetMessage.endWorkerGetMessage(builder);
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
    } else {
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

export class WorkerImpl implements Worker {
  private readonly rid: number;
  private isClosing: boolean = false;
  private readonly isClosedPromise: Promise<void>;
  public onerror?: () => void;
  public onmessage?: (data: any) => void;
  public onmessageerror?: () => void;

  constructor(specifier: string) {
    this.rid = createWorker(specifier);
    this.run();
    this.isClosedPromise = hostGetWorkerClosed(this.rid);
    this.isClosedPromise.then(() => {
      this.isClosing = true;
    });
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
