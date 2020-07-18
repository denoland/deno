// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// The specification refers to internal slots.  In most cases, ECMAScript
// Private Fields are not sufficient for these, as they are often accessed
// outside of the class itself and using a WeakMap gets really complex to hide
// this data from the public, therefore we will use unique symbols which are
// not available in the runtime.

((window) => {
  window.__streamSymbols = {
    abortAlgorithm: Symbol("abortAlgorithm"),
    abortSteps: Symbol("abortSteps"),
    asyncIteratorReader: Symbol("asyncIteratorReader"),
    autoAllocateChunkSize: Symbol("autoAllocateChunkSize"),
    backpressure: Symbol("backpressure"),
    backpressureChangePromise: Symbol("backpressureChangePromise"),
    byobRequest: Symbol("byobRequest"),
    cancelAlgorithm: Symbol("cancelAlgorithm"),
    cancelSteps: Symbol("cancelSteps"),
    closeAlgorithm: Symbol("closeAlgorithm"),
    closedPromise: Symbol("closedPromise"),
    closeRequest: Symbol("closeRequest"),
    closeRequested: Symbol("closeRequested"),
    controlledReadableByteStream: Symbol(
      "controlledReadableByteStream",
    ),
    controlledReadableStream: Symbol("controlledReadableStream"),
    controlledTransformStream: Symbol("controlledTransformStream"),
    controlledWritableStream: Symbol("controlledWritableStream"),
    disturbed: Symbol("disturbed"),
    errorSteps: Symbol("errorSteps"),
    flushAlgorithm: Symbol("flushAlgorithm"),
    forAuthorCode: Symbol("forAuthorCode"),
    inFlightWriteRequest: Symbol("inFlightWriteRequest"),
    inFlightCloseRequest: Symbol("inFlightCloseRequest"),
    isFakeDetached: Symbol("isFakeDetached"),
    ownerReadableStream: Symbol("ownerReadableStream"),
    ownerWritableStream: Symbol("ownerWritableStream"),
    pendingAbortRequest: Symbol("pendingAbortRequest"),
    preventCancel: Symbol("preventCancel"),
    pullAgain: Symbol("pullAgain"),
    pullAlgorithm: Symbol("pullAlgorithm"),
    pulling: Symbol("pulling"),
    pullSteps: Symbol("pullSteps"),
    queue: Symbol("queue"),
    queueTotalSize: Symbol("queueTotalSize"),
    readable: Symbol("readable"),
    readableStreamController: Symbol("readableStreamController"),
    reader: Symbol("reader"),
    readRequests: Symbol("readRequests"),
    readyPromise: Symbol("readyPromise"),
    started: Symbol("started"),
    state: Symbol("state"),
    storedError: Symbol("storedError"),
    strategyHWM: Symbol("strategyHWM"),
    strategySizeAlgorithm: Symbol("strategySizeAlgorithm"),
    transformAlgorithm: Symbol("transformAlgorithm"),
    transformStreamController: Symbol("transformStreamController"),
    writableStreamController: Symbol("writableStreamController"),
    writeAlgorithm: Symbol("writeAlgorithm"),
    writable: Symbol("writable"),
    writer: Symbol("writer"),
    writeRequests: Symbol("writeRequests"),
  };
})(this);
