// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// The specification refers to internal slots.  In most cases, ECMAScript
// Private Fields are not sufficient for these, as they are often accessed
// outside of the class itself and using a WeakMap gets really complex to hide
// this data from the public, therefore we will use unique symbols which are
// not available in the runtime.

export const abortAlgorithm = Symbol("abortAlgorithm");
export const abortSteps = Symbol("abortSteps");
export const asyncIteratorReader = Symbol("asyncIteratorReader");
export const autoAllocateChunkSize = Symbol("autoAllocateChunkSize");
export const backpressure = Symbol("backpressure");
export const backpressureChangePromise = Symbol("backpressureChangePromise");
export const byobRequest = Symbol("byobRequest");
export const cancelAlgorithm = Symbol("cancelAlgorithm");
export const cancelSteps = Symbol("cancelSteps");
export const closeAlgorithm = Symbol("closeAlgorithm");
export const closedPromise = Symbol("closedPromise");
export const closeRequest = Symbol("closeRequest");
export const closeRequested = Symbol("closeRequested");
export const controlledReadableByteStream = Symbol(
  "controlledReadableByteStream"
);
export const controlledReadableStream = Symbol("controlledReadableStream");
export const controlledTransformStream = Symbol("controlledTransformStream");
export const controlledWritableStream = Symbol("controlledWritableStream");
export const disturbed = Symbol("disturbed");
export const errorSteps = Symbol("errorSteps");
export const flushAlgorithm = Symbol("flushAlgorithm");
export const forAuthorCode = Symbol("forAuthorCode");
export const inFlightWriteRequest = Symbol("inFlightWriteRequest");
export const inFlightCloseRequest = Symbol("inFlightCloseRequest");
export const isFakeDetached = Symbol("isFakeDetached");
export const ownerReadableStream = Symbol("ownerReadableStream");
export const ownerWritableStream = Symbol("ownerWritableStream");
export const pendingAbortRequest = Symbol("pendingAbortRequest");
export const preventCancel = Symbol("preventCancel");
export const pullAgain = Symbol("pullAgain");
export const pullAlgorithm = Symbol("pullAlgorithm");
export const pulling = Symbol("pulling");
export const pullSteps = Symbol("pullSteps");
export const queue = Symbol("queue");
export const queueTotalSize = Symbol("queueTotalSize");
export const readable = Symbol("readable");
export const readableStreamController = Symbol("readableStreamController");
export const reader = Symbol("reader");
export const readRequests = Symbol("readRequests");
export const readyPromise = Symbol("readyPromise");
export const started = Symbol("started");
export const state = Symbol("state");
export const storedError = Symbol("storedError");
export const strategyHWM = Symbol("strategyHWM");
export const strategySizeAlgorithm = Symbol("strategySizeAlgorithm");
export const transformAlgorithm = Symbol("transformAlgorithm");
export const transformStreamController = Symbol("transformStreamController");
export const writableStreamController = Symbol("writableStreamController");
export const writeAlgorithm = Symbol("writeAlgorithm");
export const writable = Symbol("writable");
export const writer = Symbol("writer");
export const writeRequests = Symbol("writeRequests");
