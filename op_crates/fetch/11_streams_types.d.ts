// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// ** Internal Interfaces **

interface PendingAbortRequest {
  deferred: Deferred<void>;
  // deno-lint-ignore no-explicit-any
  reason: any;
  wasAlreadyErroring: boolean;
}

// deno-lint-ignore no-explicit-any
interface ReadRequest<R = any> {
  chunkSteps: (chunk: R) => void;
  closeSteps: () => void;
  // deno-lint-ignore no-explicit-any
  errorSteps: (error: any) => void;
}

interface ReadableByteStreamQueueEntry {
  buffer: ArrayBufferLike;
  byteOffset: number;
  byteLength: number;
}

interface ReadableStreamGetReaderOptions {
  mode?: "byob";
}

interface ReadableStreamIteratorOptions {
  preventCancel?: boolean;
}

interface ValueWithSize<T> {
  value: T;
  size: number;
}

interface VoidFunction {
  (): void;
}

// ** Ambient Definitions and Interfaces not provided by fetch **

declare function queueMicrotask(callback: VoidFunction): void;

declare namespace Deno {
  function inspect(value: unknown, options?: Record<string, unknown>): string;
}
