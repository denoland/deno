// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
/* eslint-disable @typescript-eslint/no-explicit-any */
import { sendAsync, sendSync } from "./dispatch";
import * as msg from "gen/cli/msg_generated";
import * as flatbuffers from "./flatbuffers";
import { assert, log, createResolvable, Resolvable } from "./util";
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

async function hostGetMessage(rid: number): Promise<Uint8Array | null> {
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

  return res.dataArray();
}

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
      try {
        window.onmessage(event);
      } catch (e) {
        const errorEvent: ErrorEvent = {
          message: e.message,
          filename: e.filename,
          lineno: e.lineno,
          colno: e.colno
        };
        log("error event from worker:", JSON.stringify(errorEvent));
        postMessage(errorEvent);
        console.error(e);
      }
    }

    if (!window["onmessage"]) {
      break;
    }
  }
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

type OnMessageHandler = (e: MessageEvent) => void;
type OnErrorHandler = (e: ErrorEvent) => void;

export let onmessage: OnMessageHandler = (): void => {};

export interface Worker {
  onerror?: OnErrorHandler;
  onmessage?: OnMessageHandler;
  onmessageerror?: OnMessageHandler;
  postMessage(data: any): void;
  closed: Promise<void>;
}

export class WorkerImpl implements Worker {
  private readonly rid: number;
  private isClosing: boolean = false;
  private readonly isClosedPromise: Promise<void>;
  // To prevent eating messages that had been sent by worker
  // but before `onmessage` listener is set we await
  // setting that handler.
  private onmessageSet: Resolvable<void> = createResolvable();
  private onmessageHandler?: OnMessageHandler;
  public onerror?: OnErrorHandler;
  public onmessageerror?: OnMessageHandler;

  constructor(specifier: string) {
    this.rid = createWorker(specifier);
    this.run();
    log("post run");
    this.isClosedPromise = hostGetWorkerClosed(this.rid);
    this.isClosedPromise.then(
      (): void => {
        this.isClosing = true;
      }
    );
  }

  set onmessage(fn: OnMessageHandler) {
    log("on message is being set", this.onmessageSet);
    this.onmessageHandler = fn;
    this.onmessageSet.resolve();
  }

  get closed(): Promise<void> {
    return this.isClosedPromise;
  }

  postMessage(data: any): void {
    hostPostMessage(this.rid, data);
  }

  private async run(): Promise<void> {
    while (!this.isClosing) {
      log("awaitng to receive message");
      const msg = await hostGetMessage(this.rid);
      log("received message");
      if (msg == null) {
        log("worker got null message. quitting.");
        break;
      }

      // Ensure we don't start accepting until we have a listener.
      log("running await", this.onmessageSet);
      await this.onmessageSet;
      log("await done");

      let decodedMsg;

      try {
        decodedMsg = decodeMessage(msg);
      } catch (e) {
        if (this.onmessageerror) {
          this.onmessageerror(({ data: msg } as any) as MessageEvent);
        }
        continue;
      }

      if (decodedMsg.message) {
        if (this.onerror) {
          this.onerror(decodedMsg as ErrorEvent);
        }
        continue;
      }

      if (this.onmessageHandler) {
        this.onmessageHandler(decodedMsg as MessageEvent);
      }
    }
  }
}
