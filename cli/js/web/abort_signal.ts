// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { EventImpl } from "./event.ts";
import { EventTargetImpl } from "./event_target.ts";

type AbortAlgorithm = () => void;

export const add = Symbol("add");
export const signalAbort = Symbol("signalAbort");
export const remove = Symbol("remove");

export class AbortSignalImpl extends EventTargetImpl implements AbortSignal {
  #aborted?: boolean;
  #abortAlgorithms = new Set<AbortAlgorithm>();

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  onabort: ((this: AbortSignal, e: Event) => any) | null = null;

  [add](algorithm: () => void): void {
    this.#abortAlgorithms.add(algorithm);
  }

  [signalAbort](): void {
    if (this.#aborted) {
      return;
    }
    this.#aborted = true;
    for (const algorithm of this.#abortAlgorithms) {
      algorithm();
    }
    this.#abortAlgorithms.clear();
    this.dispatchEvent(new EventImpl("abort"));
  }

  [remove](algorithm: () => void): void {
    this.#abortAlgorithms.delete(algorithm);
  }

  constructor() {
    super();

    this.addEventListener("abort", (evt: Event) => {
      const { onabort } = this;
      if (typeof onabort === "function") {
        onabort.call(this, evt);
      }
    });
  }

  get aborted(): boolean {
    return Boolean(this.#aborted);
  }

  get [Symbol.toStringTag](): string {
    return "AbortSignal";
  }
}

Object.defineProperty(AbortSignalImpl, "name", {
  value: "AbortSignal",
  configurable: true,
});
