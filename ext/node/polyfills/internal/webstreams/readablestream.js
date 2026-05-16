// Copyright 2018-2026 the Deno authors. MIT license.

// deno-lint-ignore-file

(function () {
const { core } = globalThis.__bootstrap;

const webStreams = core.loadExtScript("ext:deno_web/06_streams.js");
const streamWeb = core.loadExtScript("ext:deno_node/stream/web.js");

const {
  ReadableStream,
  ReadableStreamDefaultReader,
  ReadableStreamBYOBReader,
  ReadableStreamBYOBRequest,
  ReadableByteStreamController,
  ReadableStreamDefaultController,
} = streamWeb;

const TransferredReadableStream = ReadableStream;

const {
  isReadableStream,
  isReadableByteStreamController,
  isReadableStreamBYOBRequest,
  isReadableStreamDefaultReader,
  isReadableStreamBYOBReader,
  isReadableStreamLocked,
  readableStreamCancel,
  readableStreamClose,
  readableStreamGetNumReadRequests,
  readableStreamHasDefaultReader,
  readableStreamGetNumReadIntoRequests,
  readableStreamHasBYOBReader,
  readableStreamReaderGenericCancel,
  readableStreamReaderGenericRelease,
  readableStreamDefaultControllerClose,
  readableStreamDefaultControllerEnqueue,
  readableStreamDefaultControllerHasBackpressure,
  readableStreamDefaultControllerCanCloseOrEnqueue,
  readableStreamDefaultControllerGetDesiredSize,
  readableStreamDefaultControllerShouldCallPull,
  readableStreamDefaultControllerCallPullIfNeeded,
  readableStreamDefaultControllerClearAlgorithms,
  readableStreamDefaultControllerError,
  readableByteStreamControllerClose,
  readableByteStreamControllerCommitPullIntoDescriptor,
  readableByteStreamControllerInvalidateBYOBRequest,
  readableByteStreamControllerClearAlgorithms,
  readableByteStreamControllerClearPendingPullIntos,
  readableByteStreamControllerGetDesiredSize,
  readableByteStreamControllerShouldCallPull,
  readableByteStreamControllerHandleQueueDrain,
  readableByteStreamControllerPullInto,
  readableByteStreamControllerRespondInternal,
  readableByteStreamControllerRespond,
  readableByteStreamControllerRespondInClosedState,
  readableByteStreamControllerFillHeadPullIntoDescriptor,
  readableByteStreamControllerEnqueue,
  readableByteStreamControllerEnqueueChunkToQueue,
  readableByteStreamControllerFillPullIntoDescriptorFromQueue,
  readableByteStreamControllerProcessPullIntoDescriptorsUsingQueue,
  readableByteStreamControllerRespondInReadableState,
  readableByteStreamControllerRespondWithNewView,
  readableByteStreamControllerShiftPendingPullInto,
  readableByteStreamControllerCallPullIfNeeded,
  readableByteStreamControllerError,
  setUpReadableByteStreamController,
  setUpReadableByteStreamControllerFromSource,
  setUpReadableStreamDefaultController,
  setUpReadableStreamDefaultControllerFromSource,
  setUpReadableStreamBYOBReader,
  setUpReadableStreamDefaultReader,
  createReadableStream,
  createReadableByteStream,
} = webStreams;

const readableStreamPipeTo = webStreams.readableStreamPipeTo ??
  ((
    source,
    dest,
    preventClose = false,
    preventAbort = false,
    preventCancel = false,
    signal,
  ) =>
    source.pipeTo(dest, { preventClose, preventAbort, preventCancel, signal }));
const readableStreamTee = webStreams.readableStreamTee ??
  ((stream) => stream.tee());

const readableStreamError = webStreams.errorReadableStream;
const readableStreamFulfillReadRequest = undefined;
const readableStreamFulfillReadIntoRequest = undefined;
const readableStreamAddReadRequest = undefined;
const readableStreamAddReadIntoRequest = undefined;
const readableStreamDefaultControllerCancelSteps = undefined;
const readableStreamDefaultControllerPullSteps = undefined;
const readableByteStreamControllerCancelSteps = undefined;
const readableByteStreamControllerPullSteps = undefined;
const setupReadableByteStreamController = setUpReadableByteStreamController;
const setupReadableByteStreamControllerFromSource =
  setUpReadableByteStreamControllerFromSource;
const setupReadableStreamDefaultController =
  setUpReadableStreamDefaultController;
const setupReadableStreamDefaultControllerFromSource =
  setUpReadableStreamDefaultControllerFromSource;
const setupReadableStreamBYOBReader = setUpReadableStreamBYOBReader;
const setupReadableStreamDefaultReader = setUpReadableStreamDefaultReader;

const exportsObject = {
  ReadableStream,
  ReadableStreamDefaultReader,
  ReadableStreamBYOBReader,
  ReadableStreamBYOBRequest,
  ReadableByteStreamController,
  ReadableStreamDefaultController,
  TransferredReadableStream,
  isReadableStream,
  isReadableByteStreamController,
  isReadableStreamBYOBRequest,
  isReadableStreamDefaultReader,
  isReadableStreamBYOBReader,
  readableStreamPipeTo,
  readableStreamTee,
  isReadableStreamLocked,
  readableStreamCancel,
  readableStreamClose,
  readableStreamError,
  readableStreamHasDefaultReader,
  readableStreamGetNumReadRequests,
  readableStreamHasBYOBReader,
  readableStreamGetNumReadIntoRequests,
  readableStreamFulfillReadRequest,
  readableStreamFulfillReadIntoRequest,
  readableStreamAddReadRequest,
  readableStreamAddReadIntoRequest,
  readableStreamReaderGenericCancel,
  readableStreamReaderGenericRelease,
  readableStreamDefaultControllerClose,
  readableStreamDefaultControllerEnqueue,
  readableStreamDefaultControllerHasBackpressure,
  readableStreamDefaultControllerCanCloseOrEnqueue,
  readableStreamDefaultControllerGetDesiredSize,
  readableStreamDefaultControllerShouldCallPull,
  readableStreamDefaultControllerCallPullIfNeeded,
  readableStreamDefaultControllerClearAlgorithms,
  readableStreamDefaultControllerError,
  readableStreamDefaultControllerCancelSteps,
  readableStreamDefaultControllerPullSteps,
  setupReadableStreamDefaultController,
  setupReadableStreamDefaultControllerFromSource,
  readableByteStreamControllerClose,
  readableByteStreamControllerCommitPullIntoDescriptor,
  readableByteStreamControllerInvalidateBYOBRequest,
  readableByteStreamControllerClearAlgorithms,
  readableByteStreamControllerClearPendingPullIntos,
  readableByteStreamControllerGetDesiredSize,
  readableByteStreamControllerShouldCallPull,
  readableByteStreamControllerHandleQueueDrain,
  readableByteStreamControllerPullInto,
  readableByteStreamControllerRespondInternal,
  readableByteStreamControllerRespond,
  readableByteStreamControllerRespondInClosedState,
  readableByteStreamControllerFillHeadPullIntoDescriptor,
  readableByteStreamControllerEnqueue,
  readableByteStreamControllerEnqueueChunkToQueue,
  readableByteStreamControllerFillPullIntoDescriptorFromQueue,
  readableByteStreamControllerProcessPullIntoDescriptorsUsingQueue,
  readableByteStreamControllerRespondInReadableState,
  readableByteStreamControllerRespondWithNewView,
  readableByteStreamControllerShiftPendingPullInto,
  readableByteStreamControllerCallPullIfNeeded,
  readableByteStreamControllerError,
  readableByteStreamControllerCancelSteps,
  readableByteStreamControllerPullSteps,
  setupReadableByteStreamController,
  setupReadableByteStreamControllerFromSource,
  createReadableStream,
  createReadableByteStream,
};

return exportsObject;
})();
