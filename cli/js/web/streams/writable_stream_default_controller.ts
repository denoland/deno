// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import {
  AbortAlgorithm,
  CloseAlgorithm,
  isWritableStreamDefaultController,
  Pair,
  resetQueue,
  setFunctionName,
  SizeAlgorithm,
  WriteAlgorithm,
  writableStreamDefaultControllerClearAlgorithms,
  writableStreamDefaultControllerError,
} from "./internals.ts";
import * as sym from "./symbols.ts";
import { WritableStreamImpl } from "./writable_stream.ts";
import { customInspect } from "../console.ts";

export class WritableStreamDefaultControllerImpl<W>
  implements WritableStreamDefaultController {
  [sym.abortAlgorithm]: AbortAlgorithm;
  [sym.closeAlgorithm]: CloseAlgorithm;
  [sym.controlledWritableStream]: WritableStreamImpl;
  [sym.queue]: Array<Pair<{ chunk: W } | "close">>;
  [sym.queueTotalSize]: number;
  [sym.started]: boolean;
  [sym.strategyHWM]: number;
  [sym.strategySizeAlgorithm]: SizeAlgorithm<W>;
  [sym.writeAlgorithm]: WriteAlgorithm<W>;

  private constructor() {
    throw new TypeError(
      "WritableStreamDefaultController's constructor cannot be called."
    );
  }

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  error(e: any): void {
    if (!isWritableStreamDefaultController(this)) {
      throw new TypeError("Invalid WritableStreamDefaultController.");
    }
    const state = this[sym.controlledWritableStream][sym.state];
    if (state !== "writable") {
      return;
    }
    writableStreamDefaultControllerError(this, e);
  }

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  [sym.abortSteps](reason: any): PromiseLike<void> {
    const result = this[sym.abortAlgorithm](reason);
    writableStreamDefaultControllerClearAlgorithms(this);
    return result;
  }

  [sym.errorSteps](): void {
    resetQueue(this);
  }

  [customInspect](): string {
    return `${this.constructor.name} { }`;
  }
}

setFunctionName(
  WritableStreamDefaultControllerImpl,
  "WritableStreamDefaultController"
);
