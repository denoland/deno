// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { globalEval } from "./global_eval";

// The libdeno functions are moved so that users can't access them.
type MessageCallback = (msg: Uint8Array) => void;
export type PromiseRejectEvent =
  | "RejectWithNoHandler"
  | "HandlerAddedAfterReject"
  | "ResolveAfterResolved"
  | "RejectAfterResolved";

// TODO: Currently left as "any" due to TS symbol key issue
// See https://github.com/Microsoft/TypeScript/issues/1863
// tslint:disable-next-line:no-any
type ContextGlobalObject = any;

interface Libdeno {
  recv(cb: MessageCallback): void;

  send(control: ArrayBufferView, data?: ArrayBufferView): null | Uint8Array;

  print(x: string, isErr?: boolean): void;

  shared: ArrayBuffer;

  // tslint:disable-next-line:no-any
  makeContext(): ContextGlobalObject;

  // tslint:disable-next-line:no-any
  runInContext(env: ContextGlobalObject, code: string): [any, string | null];
}

const window = globalEval("this");
// @internal
export const libdeno = window.libdeno as Libdeno;
