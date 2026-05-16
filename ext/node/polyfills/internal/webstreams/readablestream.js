// Copyright 2018-2026 the Deno authors. MIT license.

// deno-lint-ignore-file

import { core } from "ext:core/mod.js";

const webStreams = core.loadExtScript("ext:deno_web/06_streams.js");
const streamWeb = core.loadExtScript("ext:deno_node/stream/web.js");

export const {
  ReadableStream,
  ReadableStreamDefaultReader,
  ReadableStreamBYOBReader,
  ReadableStreamBYOBRequest,
  ReadableByteStreamController,
  ReadableStreamDefaultController,
} = streamWeb;

export const TransferredReadableStream = ReadableStream;

export const {
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

export const readableStreamPipeTo = webStreams.readableStreamPipeTo ??
  ((source, dest, preventClose = false, preventAbort = false, preventCancel = false, signal) =>
    source.pipeTo(dest, { preventClose, preventAbort, preventCancel, signal }));
export const readableStreamTee = webStreams.readableStreamTee ??
  ((stream) => stream.tee());

export const readableStreamError = webStreams.errorReadableStream;
export const readableStreamFulfillReadRequest = undefined;
export const readableStreamFulfillReadIntoRequest = undefined;
export const readableStreamAddReadRequest = undefined;
export const readableStreamAddReadIntoRequest = undefined;
export const readableStreamDefaultControllerCancelSteps = undefined;
export const readableStreamDefaultControllerPullSteps = undefined;
export const readableByteStreamControllerCancelSteps = undefined;
export const readableByteStreamControllerPullSteps = undefined;
export const setupReadableByteStreamController =
  setUpReadableByteStreamController;
export const setupReadableByteStreamControllerFromSource =
  setUpReadableByteStreamControllerFromSource;
export const setupReadableStreamDefaultController =
  setUpReadableStreamDefaultController;
export const setupReadableStreamDefaultControllerFromSource =
  setUpReadableStreamDefaultControllerFromSource;
export const setupReadableStreamBYOBReader = setUpReadableStreamBYOBReader;
export const setupReadableStreamDefaultReader = setUpReadableStreamDefaultReader;

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

export default exportsObject;
