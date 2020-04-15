// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { AbortSignalImpl, signalAbort } from "./abort_signal.ts";

export class AbortControllerImpl implements AbortController {
  #signal = new AbortSignalImpl();

  get signal(): AbortSignal {
    return this.#signal;
  }

  abort(): void {
    this.#signal[signalAbort]();
  }

  get [Symbol.toStringTag](): string {
    return "AbortController";
  }
}

Object.defineProperty(AbortControllerImpl, "name", {
  value: "AbortController",
  configurable: true,
});
