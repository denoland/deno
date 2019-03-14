// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { window } from "./window";

type MessageCallback = (msg: Uint8Array) => void;

// Declared in core/shared_queue.js.
interface DenoCore {
  setAsyncHandler(cb: MessageCallback): void;
  dispatch(control: Uint8Array, zeroCopy?: Uint8Array): null | Uint8Array;
}

// TODO(ry) Rename to Deno.core.shared and Deno.core.setAsyncHandler.
export const DenoCore = window.DenoCore as DenoCore;
