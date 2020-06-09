// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// This code closely follows the WHATWG Stream Specification
// See: https://streams.spec.whatwg.org/
//
// There are some parts that are not fully implemented, and there are some
// comments which point to steps of the specification that are not implemented.
//

/* eslint-disable @typescript-eslint/no-explicit-any,require-await */
import { ReadableByteStreamControllerImpl } from "./readable_byte_stream_controller.ts";
import { ReadableStreamDefaultControllerImpl } from "./readable_stream_default_controller.ts";
import { ReadableStreamDefaultReaderImpl } from "./readable_stream_default_reader.ts";
import { ReadableStreamImpl } from "./readable_stream.ts";
import * as sym from "./symbols.ts";
import { TransformStreamImpl } from "./transform_stream.ts";
import { TransformStreamDefaultControllerImpl } from "./transform_stream_default_controller.ts";
import { WritableStreamDefaultControllerImpl } from "./writable_stream_default_controller.ts";
import { WritableStreamDefaultWriterImpl } from "./writable_stream_default_writer.ts";
import { WritableStreamImpl } from "./writable_stream.ts";
import { AbortSignalImpl } from "../abort_signal.ts";
import { DOMExceptionImpl as DOMException } from "../dom_exception.ts";
import { cloneValue } from "../util.ts";
import { assert, AssertionError } from "../../util.ts";

export type AbortAlgorithm = (reason?: any) => PromiseLike<void>;
export interface AbortRequest {
  promise: Deferred<void>;
  reason?: any;
  wasAlreadyErroring: boolean;
}
export interface BufferQueueItem extends Pair<ArrayBuffer | SharedArrayBuffer> {
  offset: number;
}
export type CancelAlgorithm = (reason?: any) => PromiseLike<void>;
export type CloseAlgorithm = () => PromiseLike<void>;
type Container<R = any> = {
  [sym.queue]: Array<Pair<R> | BufferQueueItem>;
  [sym.queueTotalSize]: number;
};
export type FlushAlgorithm = () => Promise<void>;
export type Pair<R> = { value: R; size: number };
export type PullAlgorithm = () => PromiseLike<void>;
export type SizeAlgorithm<T> = (chunk: T) => number;
export type StartAlgorithm = () => void | PromiseLike<void>;
export type TransformAlgorithm<I> = (chunk: I) => Promise<void>;
export type WriteAlgorithm<W> = (chunk: W) => Promise<void>;
export interface Deferred<T> {
  promise: Promise<T>;
  resolve?: (value?: T | PromiseLike<T>) => void;
  reject?: (reason?: any) => void;
}

export interface ReadableStreamGenericReader<R = any>
  extends ReadableStreamReader<R> {
  [sym.closedPromise]: Deferred<void>;
  [sym.forAuthorCode]: boolean;
  [sym.ownerReadableStream]: ReadableStreamImpl<R>;
  [sym.readRequests]: Array<Deferred<ReadableStreamReadResult<R>>>;
}

export interface ReadableStreamAsyncIterator<T = any> extends AsyncIterator<T> {
  [sym.asyncIteratorReader]: ReadableStreamDefaultReaderImpl<T>;
  [sym.preventCancel]: boolean;
  return(value?: any | PromiseLike<any>): Promise<IteratorResult<T, any>>;
}

export function acquireReadableStreamDefaultReader<T>(
  stream: ReadableStreamImpl<T>,
  forAuthorCode = false
): ReadableStreamDefaultReaderImpl<T> {
  const reader = new ReadableStreamDefaultReaderImpl(stream);
  reader[sym.forAuthorCode] = forAuthorCode;
  return reader;
}

export function acquireWritableStreamDefaultWriter<W>(
  stream: WritableStreamImpl<W>
): WritableStreamDefaultWriterImpl<W> {
  return new WritableStreamDefaultWriterImpl(stream);
}

export function call<F extends (...args: any[]) => any>(
  fn: F,
  v: ThisType<F>,
  args: Parameters<F>
): ReturnType<F> {
  return Function.prototype.apply.call(fn, v, args);
}

function createAlgorithmFromUnderlyingMethod<
  O extends UnderlyingByteSource | UnderlyingSource | Transformer,
  P extends keyof O
>(
  underlyingObject: O,
  methodName: P,
  algoArgCount: 0,
  ...extraArgs: any[]
): () => Promise<void>;
function createAlgorithmFromUnderlyingMethod<
  O extends UnderlyingByteSource | UnderlyingSource | Transformer,
  P extends keyof O
>(
  underlyingObject: O,
  methodName: P,
  algoArgCount: 1,
  ...extraArgs: any[]
): (arg: any) => Promise<void>;
function createAlgorithmFromUnderlyingMethod<
  O extends UnderlyingByteSource | UnderlyingSource | Transformer,
  P extends keyof O
>(
  underlyingObject: O,
  methodName: P,
  algoArgCount: 0 | 1,
  ...extraArgs: any[]
): (() => Promise<void>) | ((arg: any) => Promise<void>) {
  const method = underlyingObject[methodName];
  if (method) {
    if (!isCallable(method)) {
      throw new TypeError("method is not callable");
    }
    if (algoArgCount === 0) {
      return async (): Promise<void> =>
        call(method, underlyingObject, extraArgs as any);
    } else {
      return async (arg: any): Promise<void> => {
        const fullArgs = [arg, ...extraArgs];
        return call(method, underlyingObject, fullArgs as any);
      };
    }
  }
  return async (): Promise<void> => undefined;
}

function createReadableStream<T>(
  startAlgorithm: StartAlgorithm,
  pullAlgorithm: PullAlgorithm,
  cancelAlgorithm: CancelAlgorithm,
  highWaterMark = 1,
  sizeAlgorithm: SizeAlgorithm<T> = (): number => 1
): ReadableStreamImpl<T> {
  highWaterMark = validateAndNormalizeHighWaterMark(highWaterMark);
  const stream: ReadableStreamImpl<T> = Object.create(
    ReadableStreamImpl.prototype
  );
  initializeReadableStream(stream);
  const controller: ReadableStreamDefaultControllerImpl<T> = Object.create(
    ReadableStreamDefaultControllerImpl.prototype
  );
  setUpReadableStreamDefaultController(
    stream,
    controller,
    startAlgorithm,
    pullAlgorithm,
    cancelAlgorithm,
    highWaterMark,
    sizeAlgorithm
  );
  return stream;
}

function createWritableStream<W>(
  startAlgorithm: StartAlgorithm,
  writeAlgorithm: WriteAlgorithm<W>,
  closeAlgorithm: CloseAlgorithm,
  abortAlgorithm: AbortAlgorithm,
  highWaterMark = 1,
  sizeAlgorithm: SizeAlgorithm<W> = (): number => 1
): WritableStreamImpl<W> {
  highWaterMark = validateAndNormalizeHighWaterMark(highWaterMark);
  const stream = Object.create(WritableStreamImpl.prototype);
  initializeWritableStream(stream);
  const controller = Object.create(
    WritableStreamDefaultControllerImpl.prototype
  );
  setUpWritableStreamDefaultController(
    stream,
    controller,
    startAlgorithm,
    writeAlgorithm,
    closeAlgorithm,
    abortAlgorithm,
    highWaterMark,
    sizeAlgorithm
  );
  return stream;
}

export function dequeueValue<R>(container: Container<R>): R {
  assert(sym.queue in container && sym.queueTotalSize in container);
  assert(container[sym.queue].length);
  const pair = container[sym.queue].shift()!;
  container[sym.queueTotalSize] -= pair.size;
  if (container[sym.queueTotalSize] <= 0) {
    container[sym.queueTotalSize] = 0;
  }
  return pair.value as R;
}

function enqueueValueWithSize<R>(
  container: Container<R>,
  value: R,
  size: number
): void {
  assert(sym.queue in container && sym.queueTotalSize in container);
  size = Number(size);
  if (!isFiniteNonNegativeNumber(size)) {
    throw new RangeError("size must be a finite non-negative number.");
  }
  container[sym.queue].push({ value, size });
  container[sym.queueTotalSize] += size;
}

/** Non-spec mechanism to "unwrap" a promise and store it to be resolved
 * later. */
export function getDeferred<T>(): Required<Deferred<T>> {
  let resolve: (value?: T | PromiseLike<T>) => void;
  let reject: (reason?: any) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve: resolve!, reject: reject! };
}

export function initializeReadableStream<R>(
  stream: ReadableStreamImpl<R>
): void {
  stream[sym.state] = "readable";
  stream[sym.reader] = stream[sym.storedError] = undefined;
  stream[sym.disturbed] = false;
}

export function initializeTransformStream<I, O>(
  stream: TransformStreamImpl<I, O>,
  startPromise: Promise<void>,
  writableHighWaterMark: number,
  writableSizeAlgorithm: SizeAlgorithm<I>,
  readableHighWaterMark: number,
  readableSizeAlgorithm: SizeAlgorithm<O>
): void {
  const startAlgorithm = (): Promise<void> => startPromise;
  const writeAlgorithm = (chunk: any): Promise<void> =>
    transformStreamDefaultSinkWriteAlgorithm(stream, chunk);
  const abortAlgorithm = (reason: any): Promise<void> =>
    transformStreamDefaultSinkAbortAlgorithm(stream, reason);
  const closeAlgorithm = (): Promise<void> =>
    transformStreamDefaultSinkCloseAlgorithm(stream);
  stream[sym.writable] = createWritableStream(
    startAlgorithm,
    writeAlgorithm,
    closeAlgorithm,
    abortAlgorithm,
    writableHighWaterMark,
    writableSizeAlgorithm
  );
  const pullAlgorithm = (): PromiseLike<void> =>
    transformStreamDefaultSourcePullAlgorithm(stream);
  const cancelAlgorithm = (reason: any): Promise<void> => {
    transformStreamErrorWritableAndUnblockWrite(stream, reason);
    return Promise.resolve(undefined);
  };
  stream[sym.readable] = createReadableStream(
    startAlgorithm,
    pullAlgorithm,
    cancelAlgorithm,
    readableHighWaterMark,
    readableSizeAlgorithm
  );
  stream[sym.backpressure] = stream[sym.backpressureChangePromise] = undefined;
  transformStreamSetBackpressure(stream, true);
  Object.defineProperty(stream, sym.transformStreamController, {
    value: undefined,
    configurable: true,
  });
}

export function initializeWritableStream<W>(
  stream: WritableStreamImpl<W>
): void {
  stream[sym.state] = "writable";
  stream[sym.storedError] = stream[sym.writer] = stream[
    sym.writableStreamController
  ] = stream[sym.inFlightWriteRequest] = stream[sym.closeRequest] = stream[
    sym.inFlightCloseRequest
  ] = stream[sym.pendingAbortRequest] = undefined;
  stream[sym.writeRequests] = [];
  stream[sym.backpressure] = false;
}

export function invokeOrNoop<O extends Record<string, any>, P extends keyof O>(
  o: O,
  p: P,
  ...args: Parameters<O[P]>
): ReturnType<O[P]> | undefined {
  assert(o);
  const method = o[p];
  if (!method) {
    return undefined;
  }
  return call(method, o, args);
}

function isCallable(value: unknown): value is (...args: any) => any {
  return typeof value === "function";
}

export function isDetachedBuffer(value: object): boolean {
  return sym.isFakeDetached in value;
}

function isFiniteNonNegativeNumber(v: unknown): v is number {
  return Number.isFinite(v) && (v as number) >= 0;
}

export function isReadableByteStreamController(
  x: unknown
): x is ReadableByteStreamControllerImpl {
  return typeof x !== "object" ||
    x === null ||
    !(sym.controlledReadableByteStream in x)
    ? false
    : true;
}

export function isReadableStream(x: unknown): x is ReadableStreamImpl {
  return typeof x !== "object" ||
    x === null ||
    !(sym.readableStreamController in x)
    ? false
    : true;
}

export function isReadableStreamAsyncIterator(
  x: unknown
): x is ReadableStreamAsyncIterator<any> {
  if (typeof x !== "object" || x === null) {
    return false;
  }
  if (!(sym.asyncIteratorReader in x)) {
    return false;
  }
  return true;
}

export function isReadableStreamDefaultController(
  x: unknown
): x is ReadableStreamDefaultControllerImpl {
  return typeof x !== "object" ||
    x === null ||
    !(sym.controlledReadableStream in x)
    ? false
    : true;
}

export function isReadableStreamDefaultReader<T>(
  x: unknown
): x is ReadableStreamDefaultReaderImpl<T> {
  return typeof x !== "object" || x === null || !(sym.readRequests in x)
    ? false
    : true;
}

export function isReadableStreamLocked(stream: ReadableStreamImpl): boolean {
  assert(isReadableStream(stream));
  return stream[sym.reader] ? true : false;
}

export function isReadableStreamDisturbed(stream: ReadableStream): boolean {
  assert(isReadableStream(stream));
  return stream[sym.disturbed] ? true : false;
}

export function isTransformStream(
  x: unknown
): x is TransformStreamImpl<any, any> {
  return typeof x !== "object" ||
    x === null ||
    !(sym.transformStreamController in x)
    ? false
    : true;
}

export function isTransformStreamDefaultController(
  x: unknown
): x is TransformStreamDefaultControllerImpl<any, any> {
  return typeof x !== "object" ||
    x === null ||
    !(sym.controlledTransformStream in x)
    ? false
    : true;
}

export function isUnderlyingByteSource(
  underlyingSource: UnderlyingByteSource | UnderlyingSource
): underlyingSource is UnderlyingByteSource {
  const { type } = underlyingSource;
  const typeString = String(type);
  return typeString === "bytes";
}

export function isWritableStream(x: unknown): x is WritableStreamImpl {
  return typeof x !== "object" ||
    x === null ||
    !(sym.writableStreamController in x)
    ? false
    : true;
}

export function isWritableStreamDefaultController(
  x: unknown
): x is WritableStreamDefaultControllerImpl<any> {
  return typeof x !== "object" ||
    x === null ||
    !(sym.controlledWritableStream in x)
    ? false
    : true;
}

export function isWritableStreamDefaultWriter(
  x: unknown
): x is WritableStreamDefaultWriterImpl<any> {
  return typeof x !== "object" || x === null || !(sym.ownerWritableStream in x)
    ? false
    : true;
}

export function isWritableStreamLocked(stream: WritableStreamImpl): boolean {
  assert(isWritableStream(stream));
  if (stream[sym.writer] === undefined) {
    return false;
  }
  return true;
}

export function makeSizeAlgorithmFromSizeFunction<T>(
  size: QueuingStrategySizeCallback<T> | undefined
): SizeAlgorithm<T> {
  if (size === undefined) {
    return (): number => 1;
  }
  if (typeof size !== "function") {
    throw new TypeError("size must be callable.");
  }
  return (chunk: T): number => {
    return size.call(undefined, chunk);
  };
}

function peekQueueValue<T>(container: Container<T>): T | "close" {
  assert(sym.queue in container && sym.queueTotalSize in container);
  assert(container[sym.queue].length);
  const [pair] = container[sym.queue];
  return pair.value as T;
}

function readableByteStreamControllerShouldCallPull(
  controller: ReadableByteStreamControllerImpl
): boolean {
  const stream = controller[sym.controlledReadableByteStream];
  if (
    stream[sym.state] !== "readable" ||
    controller[sym.closeRequested] ||
    !controller[sym.started]
  ) {
    return false;
  }
  if (
    readableStreamHasDefaultReader(stream) &&
    readableStreamGetNumReadRequests(stream) > 0
  ) {
    return true;
  }
  // 3.13.25.6 If ! ReadableStreamHasBYOBReader(stream) is true and !
  //            ReadableStreamGetNumReadIntoRequests(stream) > 0, return true.
  const desiredSize = readableByteStreamControllerGetDesiredSize(controller);
  assert(desiredSize !== null);
  if (desiredSize > 0) {
    return true;
  }
  return false;
}

export function readableByteStreamControllerCallPullIfNeeded(
  controller: ReadableByteStreamControllerImpl
): void {
  const shouldPull = readableByteStreamControllerShouldCallPull(controller);
  if (!shouldPull) {
    return;
  }
  if (controller[sym.pulling]) {
    controller[sym.pullAgain] = true;
    return;
  }
  assert(controller[sym.pullAgain] === false);
  controller[sym.pulling] = true;
  const pullPromise = controller[sym.pullAlgorithm]();
  setPromiseIsHandledToTrue(
    pullPromise.then(
      () => {
        controller[sym.pulling] = false;
        if (controller[sym.pullAgain]) {
          controller[sym.pullAgain] = false;
          readableByteStreamControllerCallPullIfNeeded(controller);
        }
      },
      (e) => {
        readableByteStreamControllerError(controller, e);
      }
    )
  );
}

export function readableByteStreamControllerClearAlgorithms(
  controller: ReadableByteStreamControllerImpl
): void {
  (controller as any)[sym.pullAlgorithm] = undefined;
  (controller as any)[sym.cancelAlgorithm] = undefined;
}

export function readableByteStreamControllerClose(
  controller: ReadableByteStreamControllerImpl
): void {
  const stream = controller[sym.controlledReadableByteStream];
  if (controller[sym.closeRequested] || stream[sym.state] !== "readable") {
    return;
  }
  if (controller[sym.queueTotalSize] > 0) {
    controller[sym.closeRequested] = true;
    return;
  }
  // 3.13.6.4 If controller.[[pendingPullIntos]] is not empty, (BYOB Support)
  readableByteStreamControllerClearAlgorithms(controller);
  readableStreamClose(stream);
}

export function readableByteStreamControllerEnqueue(
  controller: ReadableByteStreamControllerImpl,
  chunk: ArrayBufferView
): void {
  const stream = controller[sym.controlledReadableByteStream];
  if (controller[sym.closeRequested] || stream[sym.state] !== "readable") {
    return;
  }
  const { buffer, byteOffset, byteLength } = chunk;
  const transferredBuffer = transferArrayBuffer(buffer);
  if (readableStreamHasDefaultReader(stream)) {
    if (readableStreamGetNumReadRequests(stream) === 0) {
      readableByteStreamControllerEnqueueChunkToQueue(
        controller,
        transferredBuffer,
        byteOffset,
        byteLength
      );
    } else {
      assert(controller[sym.queue].length === 0);
      const transferredView = new Uint8Array(
        transferredBuffer,
        byteOffset,
        byteLength
      );
      readableStreamFulfillReadRequest(stream, transferredView, false);
    }
    // 3.13.9.8 Otherwise, if ! ReadableStreamHasBYOBReader(stream) is true
  } else {
    assert(!isReadableStreamLocked(stream));
    readableByteStreamControllerEnqueueChunkToQueue(
      controller,
      transferredBuffer,
      byteOffset,
      byteLength
    );
  }
  readableByteStreamControllerCallPullIfNeeded(controller);
}

function readableByteStreamControllerEnqueueChunkToQueue(
  controller: ReadableByteStreamControllerImpl,
  buffer: ArrayBuffer | SharedArrayBuffer,
  byteOffset: number,
  byteLength: number
): void {
  controller[sym.queue].push({
    value: buffer,
    offset: byteOffset,
    size: byteLength,
  });
  controller[sym.queueTotalSize] += byteLength;
}

export function readableByteStreamControllerError(
  controller: ReadableByteStreamControllerImpl,
  e: any
): void {
  const stream = controller[sym.controlledReadableByteStream];
  if (stream[sym.state] !== "readable") {
    return;
  }
  // 3.13.11.3 Perform ! ReadableByteStreamControllerClearPendingPullIntos(controller).
  resetQueue(controller);
  readableByteStreamControllerClearAlgorithms(controller);
  readableStreamError(stream, e);
}

export function readableByteStreamControllerGetDesiredSize(
  controller: ReadableByteStreamControllerImpl
): number | null {
  const stream = controller[sym.controlledReadableByteStream];
  const state = stream[sym.state];
  if (state === "errored") {
    return null;
  }
  if (state === "closed") {
    return 0;
  }
  return controller[sym.strategyHWM] - controller[sym.queueTotalSize];
}

export function readableByteStreamControllerHandleQueueDrain(
  controller: ReadableByteStreamControllerImpl
): void {
  assert(
    controller[sym.controlledReadableByteStream][sym.state] === "readable"
  );
  if (controller[sym.queueTotalSize] === 0 && controller[sym.closeRequested]) {
    readableByteStreamControllerClearAlgorithms(controller);
    readableStreamClose(controller[sym.controlledReadableByteStream]);
  } else {
    readableByteStreamControllerCallPullIfNeeded(controller);
  }
}

export function readableStreamAddReadRequest<R>(
  stream: ReadableStreamImpl<R>
): Promise<ReadableStreamReadResult<R>> {
  assert(isReadableStreamDefaultReader(stream[sym.reader]));
  assert(stream[sym.state] === "readable");
  const promise = getDeferred<ReadableStreamReadResult<R>>();
  stream[sym.reader]![sym.readRequests].push(promise);
  return promise.promise;
}

export function readableStreamCancel<T>(
  stream: ReadableStreamImpl<T>,
  reason: any
): Promise<void> {
  stream[sym.disturbed] = true;
  if (stream[sym.state] === "closed") {
    return Promise.resolve();
  }
  if (stream[sym.state] === "errored") {
    return Promise.reject(stream[sym.storedError]);
  }
  readableStreamClose(stream);
  return stream[sym.readableStreamController]![sym.cancelSteps](reason).then(
    () => undefined
  ) as Promise<void>;
}

export function readableStreamClose<T>(stream: ReadableStreamImpl<T>): void {
  assert(stream[sym.state] === "readable");
  stream[sym.state] = "closed";
  const reader = stream[sym.reader];
  if (!reader) {
    return;
  }
  if (isReadableStreamDefaultReader<T>(reader)) {
    for (const readRequest of reader[sym.readRequests]) {
      assert(readRequest.resolve);
      readRequest.resolve(
        readableStreamCreateReadResult<T>(
          undefined,
          true,
          reader[sym.forAuthorCode]
        )
      );
    }
    reader[sym.readRequests] = [];
  }
  const resolve = reader[sym.closedPromise].resolve;
  assert(resolve);
  resolve();
}

export function readableStreamCreateReadResult<T>(
  value: T | undefined,
  done: boolean,
  forAuthorCode: boolean
): ReadableStreamReadResult<T> {
  const prototype = forAuthorCode ? Object.prototype : null;
  assert(typeof done === "boolean");
  const obj: ReadableStreamReadResult<T> = Object.create(prototype);
  Object.defineProperties(obj, {
    value: { value, writable: true, enumerable: true, configurable: true },
    done: { value: done, writable: true, enumerable: true, configurable: true },
  });
  return obj;
}

export function readableStreamDefaultControllerCallPullIfNeeded<T>(
  controller: ReadableStreamDefaultControllerImpl<T>
): void {
  const shouldPull = readableStreamDefaultControllerShouldCallPull(controller);
  if (!shouldPull) {
    return;
  }
  if (controller[sym.pulling]) {
    controller[sym.pullAgain] = true;
    return;
  }
  assert(controller[sym.pullAgain] === false);
  controller[sym.pulling] = true;
  const pullPromise = controller[sym.pullAlgorithm]();
  pullPromise.then(
    () => {
      controller[sym.pulling] = false;
      if (controller[sym.pullAgain]) {
        controller[sym.pullAgain] = false;
        readableStreamDefaultControllerCallPullIfNeeded(controller);
      }
    },
    (e) => {
      readableStreamDefaultControllerError(controller, e);
    }
  );
}

export function readableStreamDefaultControllerCanCloseOrEnqueue<T>(
  controller: ReadableStreamDefaultControllerImpl<T>
): boolean {
  const state = controller[sym.controlledReadableStream][sym.state];
  if (!controller[sym.closeRequested] && state === "readable") {
    return true;
  }
  return false;
}

export function readableStreamDefaultControllerClearAlgorithms<T>(
  controller: ReadableStreamDefaultControllerImpl<T>
): void {
  (controller as any)[sym.pullAlgorithm] = undefined;
  (controller as any)[sym.cancelAlgorithm] = undefined;
  (controller as any)[sym.strategySizeAlgorithm] = undefined;
}

export function readableStreamDefaultControllerClose<T>(
  controller: ReadableStreamDefaultControllerImpl<T>
): void {
  if (!readableStreamDefaultControllerCanCloseOrEnqueue(controller)) {
    return;
  }
  const stream = controller[sym.controlledReadableStream];
  controller[sym.closeRequested] = true;
  if (controller[sym.queue].length === 0) {
    readableStreamDefaultControllerClearAlgorithms(controller);
    readableStreamClose(stream);
  }
}

export function readableStreamDefaultControllerEnqueue<T>(
  controller: ReadableStreamDefaultControllerImpl<T>,
  chunk: T
): void {
  if (!readableStreamDefaultControllerCanCloseOrEnqueue(controller)) {
    return;
  }
  const stream = controller[sym.controlledReadableStream];
  if (
    isReadableStreamLocked(stream) &&
    readableStreamGetNumReadRequests(stream) > 0
  ) {
    readableStreamFulfillReadRequest(stream, chunk, false);
  } else {
    try {
      const chunkSize = controller[sym.strategySizeAlgorithm](chunk);
      enqueueValueWithSize(controller, chunk, chunkSize);
    } catch (err) {
      readableStreamDefaultControllerError(controller, err);
      throw err;
    }
  }
  readableStreamDefaultControllerCallPullIfNeeded(controller);
}

export function readableStreamDefaultControllerGetDesiredSize<T>(
  controller: ReadableStreamDefaultControllerImpl<T>
): number | null {
  const stream = controller[sym.controlledReadableStream];
  const state = stream[sym.state];
  if (state === "errored") {
    return null;
  }
  if (state === "closed") {
    return 0;
  }
  return controller[sym.strategyHWM] - controller[sym.queueTotalSize];
}

export function readableStreamDefaultControllerError<T>(
  controller: ReadableStreamDefaultControllerImpl<T>,
  e: any
): void {
  const stream = controller[sym.controlledReadableStream];
  if (stream[sym.state] !== "readable") {
    return;
  }
  resetQueue(controller);
  readableStreamDefaultControllerClearAlgorithms(controller);
  readableStreamError(stream, e);
}

function readableStreamDefaultControllerHasBackpressure<T>(
  controller: ReadableStreamDefaultControllerImpl<T>
): boolean {
  return readableStreamDefaultControllerShouldCallPull(controller)
    ? true
    : false;
}

function readableStreamDefaultControllerShouldCallPull<T>(
  controller: ReadableStreamDefaultControllerImpl<T>
): boolean {
  const stream = controller[sym.controlledReadableStream];
  if (
    !readableStreamDefaultControllerCanCloseOrEnqueue(controller) ||
    controller[sym.started] === false
  ) {
    return false;
  }
  if (
    isReadableStreamLocked(stream) &&
    readableStreamGetNumReadRequests(stream) > 0
  ) {
    return true;
  }
  const desiredSize = readableStreamDefaultControllerGetDesiredSize(controller);
  assert(desiredSize !== null);
  if (desiredSize > 0) {
    return true;
  }
  return false;
}

export function readableStreamDefaultReaderRead<R>(
  reader: ReadableStreamDefaultReaderImpl<R>
): Promise<ReadableStreamReadResult<R>> {
  const stream = reader[sym.ownerReadableStream];
  assert(stream);
  stream[sym.disturbed] = true;
  if (stream[sym.state] === "closed") {
    return Promise.resolve(
      readableStreamCreateReadResult<R>(
        undefined,
        true,
        reader[sym.forAuthorCode]
      )
    );
  }
  if (stream[sym.state] === "errored") {
    return Promise.reject(stream[sym.storedError]);
  }
  assert(stream[sym.state] === "readable");
  return (stream[
    sym.readableStreamController
  ] as ReadableStreamDefaultControllerImpl)[sym.pullSteps]();
}

export function readableStreamError(stream: ReadableStreamImpl, e: any): void {
  assert(isReadableStream(stream));
  assert(stream[sym.state] === "readable");
  stream[sym.state] = "errored";
  stream[sym.storedError] = e;
  const reader = stream[sym.reader];
  if (reader === undefined) {
    return;
  }
  if (isReadableStreamDefaultReader(reader)) {
    for (const readRequest of reader[sym.readRequests]) {
      assert(readRequest.reject);
      readRequest.reject(e);
      readRequest.reject = undefined;
      readRequest.resolve = undefined;
    }
    reader[sym.readRequests] = [];
  }
  // 3.5.6.8 Otherwise, support BYOB Reader
  reader[sym.closedPromise].reject!(e);
  reader[sym.closedPromise].reject = undefined;
  reader[sym.closedPromise].resolve = undefined;
  setPromiseIsHandledToTrue(reader[sym.closedPromise].promise);
}

export function readableStreamFulfillReadRequest<R>(
  stream: ReadableStreamImpl<R>,
  chunk: R,
  done: boolean
): void {
  const reader = stream[sym.reader]!;
  const readRequest = reader[sym.readRequests].shift()!;
  assert(readRequest.resolve);
  readRequest.resolve(
    readableStreamCreateReadResult(chunk, done, reader[sym.forAuthorCode])
  );
}

export function readableStreamGetNumReadRequests(
  stream: ReadableStreamImpl
): number {
  return stream[sym.reader]?.[sym.readRequests].length ?? 0;
}

export function readableStreamHasDefaultReader(
  stream: ReadableStreamImpl
): boolean {
  const reader = stream[sym.reader];
  return reader === undefined || !isReadableStreamDefaultReader(reader)
    ? false
    : true;
}

export function readableStreamPipeTo<T>(
  source: ReadableStreamImpl<T>,
  dest: WritableStreamImpl<T>,
  preventClose: boolean,
  preventAbort: boolean,
  preventCancel: boolean,
  signal: AbortSignalImpl | undefined
): Promise<void> {
  assert(isReadableStream(source));
  assert(isWritableStream(dest));
  assert(
    typeof preventClose === "boolean" &&
      typeof preventAbort === "boolean" &&
      typeof preventCancel === "boolean"
  );
  assert(signal === undefined || signal instanceof AbortSignalImpl);
  assert(!isReadableStreamLocked(source));
  assert(!isWritableStreamLocked(dest));
  const reader = acquireReadableStreamDefaultReader(source);
  const writer = acquireWritableStreamDefaultWriter(dest);
  source[sym.disturbed] = true;
  let shuttingDown = false;
  const promise = getDeferred<void>();
  let abortAlgorithm: () => void;
  if (signal) {
    abortAlgorithm = (): void => {
      const error = new DOMException("Abort signal received.", "AbortSignal");
      const actions: Array<() => Promise<void>> = [];
      if (!preventAbort) {
        actions.push(() => {
          if (dest[sym.state] === "writable") {
            return writableStreamAbort(dest, error);
          } else {
            return Promise.resolve(undefined);
          }
        });
      }
      if (!preventCancel) {
        actions.push(() => {
          if (source[sym.state] === "readable") {
            return readableStreamCancel(source, error);
          } else {
            return Promise.resolve(undefined);
          }
        });
      }
      shutdownWithAction(
        () => Promise.all(actions.map((action) => action())),
        true,
        error
      );
    };
    if (signal.aborted) {
      abortAlgorithm();
      return promise.promise;
    }
    signal.addEventListener("abort", abortAlgorithm);
  }

  let currentWrite = Promise.resolve();

  // At this point, the spec becomes non-specific and vague.  Most of the rest
  // of this code is based on the reference implementation that is part of the
  // specification.  This is why the functions are only scoped to this function
  // to ensure they don't leak into the spec compliant parts.

  function isOrBecomesClosed(
    stream: ReadableStreamImpl | WritableStreamImpl,
    promise: Promise<void>,
    action: () => void
  ): void {
    if (stream[sym.state] === "closed") {
      action();
    } else {
      setPromiseIsHandledToTrue(promise.then(action));
    }
  }

  function isOrBecomesErrored(
    stream: ReadableStreamImpl | WritableStreamImpl,
    promise: Promise<void>,
    action: (error: any) => void
  ): void {
    if (stream[sym.state] === "errored") {
      action(stream[sym.storedError]);
    } else {
      setPromiseIsHandledToTrue(promise.catch((error) => action(error)));
    }
  }

  function finalize(isError?: boolean, error?: any): void {
    writableStreamDefaultWriterRelease(writer);
    readableStreamReaderGenericRelease(reader);

    if (signal) {
      signal.removeEventListener("abort", abortAlgorithm);
    }
    if (isError) {
      promise.reject(error);
    } else {
      promise.resolve();
    }
  }

  function waitForWritesToFinish(): Promise<void> {
    const oldCurrentWrite = currentWrite;
    return currentWrite.then(() =>
      oldCurrentWrite !== currentWrite ? waitForWritesToFinish() : undefined
    );
  }

  function shutdownWithAction(
    action: () => Promise<any>,
    originalIsError?: boolean,
    originalError?: any
  ): void {
    function doTheRest(): void {
      setPromiseIsHandledToTrue(
        action().then(
          () => finalize(originalIsError, originalError),
          (newError) => finalize(true, newError)
        )
      );
    }

    if (shuttingDown) {
      return;
    }
    shuttingDown = true;

    if (
      dest[sym.state] === "writable" &&
      writableStreamCloseQueuedOrInFlight(dest) === false
    ) {
      setPromiseIsHandledToTrue(waitForWritesToFinish().then(doTheRest));
    } else {
      doTheRest();
    }
  }

  function shutdown(isError: boolean, error?: any): void {
    if (shuttingDown) {
      return;
    }
    shuttingDown = true;

    if (
      dest[sym.state] === "writable" &&
      !writableStreamCloseQueuedOrInFlight(dest)
    ) {
      setPromiseIsHandledToTrue(
        waitForWritesToFinish().then(() => finalize(isError, error))
      );
    }
    finalize(isError, error);
  }

  function pipeStep(): Promise<boolean> {
    if (shuttingDown) {
      return Promise.resolve(true);
    }
    return writer[sym.readyPromise].promise.then(() => {
      return readableStreamDefaultReaderRead(reader).then(({ value, done }) => {
        if (done === true) {
          return true;
        }
        currentWrite = writableStreamDefaultWriterWrite(
          writer,
          value!
        ).then(undefined, () => {});
        return false;
      });
    });
  }

  function pipeLoop(): Promise<void> {
    return new Promise((resolveLoop, rejectLoop) => {
      function next(done: boolean): void {
        if (done) {
          resolveLoop(undefined);
        } else {
          setPromiseIsHandledToTrue(pipeStep().then(next, rejectLoop));
        }
      }
      next(false);
    });
  }

  isOrBecomesErrored(
    source,
    reader[sym.closedPromise].promise,
    (storedError) => {
      if (!preventAbort) {
        shutdownWithAction(
          () => writableStreamAbort(dest, storedError),
          true,
          storedError
        );
      } else {
        shutdown(true, storedError);
      }
    }
  );

  isOrBecomesErrored(dest, writer[sym.closedPromise].promise, (storedError) => {
    if (!preventCancel) {
      shutdownWithAction(
        () => readableStreamCancel(source, storedError),
        true,
        storedError
      );
    } else {
      shutdown(true, storedError);
    }
  });

  isOrBecomesClosed(source, reader[sym.closedPromise].promise, () => {
    if (!preventClose) {
      shutdownWithAction(() =>
        writableStreamDefaultWriterCloseWithErrorPropagation(writer)
      );
    }
  });

  if (
    writableStreamCloseQueuedOrInFlight(dest) ||
    dest[sym.state] === "closed"
  ) {
    const destClosed = new TypeError(
      "The destination writable stream closed before all data could be piped to it."
    );
    if (!preventCancel) {
      shutdownWithAction(
        () => readableStreamCancel(source, destClosed),
        true,
        destClosed
      );
    } else {
      shutdown(true, destClosed);
    }
  }

  setPromiseIsHandledToTrue(pipeLoop());
  return promise.promise;
}

export function readableStreamReaderGenericCancel<R = any>(
  reader: ReadableStreamGenericReader<R>,
  reason: any
): Promise<void> {
  const stream = reader[sym.ownerReadableStream];
  assert(stream);
  return readableStreamCancel(stream, reason);
}

export function readableStreamReaderGenericInitialize<R = any>(
  reader: ReadableStreamGenericReader<R>,
  stream: ReadableStreamImpl<R>
): void {
  reader[sym.forAuthorCode] = true;
  reader[sym.ownerReadableStream] = stream;
  stream[sym.reader] = reader;
  if (stream[sym.state] === "readable") {
    reader[sym.closedPromise] = getDeferred();
  } else if (stream[sym.state] === "closed") {
    reader[sym.closedPromise] = { promise: Promise.resolve() };
  } else {
    assert(stream[sym.state] === "errored");
    reader[sym.closedPromise] = {
      promise: Promise.reject(stream[sym.storedError]),
    };
    setPromiseIsHandledToTrue(reader[sym.closedPromise].promise);
  }
}

export function readableStreamReaderGenericRelease<R = any>(
  reader: ReadableStreamGenericReader<R>
): void {
  assert(reader[sym.ownerReadableStream]);
  assert(reader[sym.ownerReadableStream][sym.reader] === reader);
  const closedPromise = reader[sym.closedPromise];
  if (reader[sym.ownerReadableStream][sym.state] === "readable") {
    assert(closedPromise.reject);
    closedPromise.reject(new TypeError("ReadableStream state is readable."));
  } else {
    closedPromise.promise = Promise.reject(new TypeError("Reading is closed."));
    delete closedPromise.reject;
    delete closedPromise.resolve;
  }
  setPromiseIsHandledToTrue(closedPromise.promise);
  reader[sym.ownerReadableStream][sym.reader] = undefined;
  (reader as any)[sym.ownerReadableStream] = undefined;
}

export function readableStreamTee<T>(
  stream: ReadableStreamImpl<T>,
  cloneForBranch2: boolean
): [ReadableStreamImpl<T>, ReadableStreamImpl<T>] {
  assert(isReadableStream(stream));
  assert(typeof cloneForBranch2 === "boolean");
  const reader = acquireReadableStreamDefaultReader(stream);
  let reading = false;
  let canceled1 = false;
  let canceled2 = false;
  let reason1: any = undefined;
  let reason2: any = undefined;
  /* eslint-disable prefer-const */
  let branch1: ReadableStreamImpl<T>;
  let branch2: ReadableStreamImpl<T>;
  /* eslint-enable prefer-const */
  const cancelPromise = getDeferred<void>();
  const pullAlgorithm = (): PromiseLike<void> => {
    if (reading) {
      return Promise.resolve();
    }
    reading = true;
    const readPromise = readableStreamDefaultReaderRead(reader).then(
      (result) => {
        reading = false;
        assert(typeof result === "object");
        const { done } = result;
        assert(typeof done === "boolean");
        if (done) {
          if (!canceled1) {
            readableStreamDefaultControllerClose(
              branch1[
                sym.readableStreamController
              ] as ReadableStreamDefaultControllerImpl
            );
          }
          if (!canceled2) {
            readableStreamDefaultControllerClose(
              branch2[
                sym.readableStreamController
              ] as ReadableStreamDefaultControllerImpl
            );
          }
          return;
        }
        const { value } = result;
        const value1 = value!;
        let value2 = value!;
        if (!canceled2 && cloneForBranch2) {
          value2 = cloneValue(value2);
        }
        if (!canceled1) {
          readableStreamDefaultControllerEnqueue(
            branch1[
              sym.readableStreamController
            ] as ReadableStreamDefaultControllerImpl,
            value1
          );
        }
        if (!canceled2) {
          readableStreamDefaultControllerEnqueue(
            branch2[
              sym.readableStreamController
            ] as ReadableStreamDefaultControllerImpl,
            value2
          );
        }
      }
    );
    setPromiseIsHandledToTrue(readPromise);
    return Promise.resolve();
  };
  const cancel1Algorithm = (reason?: any): PromiseLike<void> => {
    canceled1 = true;
    reason1 = reason;
    if (canceled2) {
      const compositeReason = [reason1, reason2];
      const cancelResult = readableStreamCancel(stream, compositeReason);
      cancelPromise.resolve(cancelResult);
    }
    return cancelPromise.promise;
  };
  const cancel2Algorithm = (reason?: any): PromiseLike<void> => {
    canceled2 = true;
    reason2 = reason;
    if (canceled1) {
      const compositeReason = [reason1, reason2];
      const cancelResult = readableStreamCancel(stream, compositeReason);
      cancelPromise.resolve(cancelResult);
    }
    return cancelPromise.promise;
  };
  const startAlgorithm = (): void => undefined;
  branch1 = createReadableStream(
    startAlgorithm,
    pullAlgorithm,
    cancel1Algorithm
  );
  branch2 = createReadableStream(
    startAlgorithm,
    pullAlgorithm,
    cancel2Algorithm
  );
  setPromiseIsHandledToTrue(
    reader[sym.closedPromise].promise.catch((r) => {
      readableStreamDefaultControllerError(
        branch1[
          sym.readableStreamController
        ] as ReadableStreamDefaultControllerImpl,
        r
      );
      readableStreamDefaultControllerError(
        branch2[
          sym.readableStreamController
        ] as ReadableStreamDefaultControllerImpl,
        r
      );
    })
  );
  return [branch1, branch2];
}

export function resetQueue<R>(container: Container<R>): void {
  assert(sym.queue in container && sym.queueTotalSize in container);
  container[sym.queue] = [];
  container[sym.queueTotalSize] = 0;
}

/** An internal function which provides a function name for some generated
 * functions, so stack traces are a bit more readable. */
export function setFunctionName(fn: Function, value: string): void {
  Object.defineProperty(fn, "name", { value, configurable: true });
}

/** An internal function which mimics the behavior of setting the promise to
 * handled in JavaScript.  In this situation, an assertion failure, which
 * shouldn't happen will get thrown, instead of swallowed. */
export function setPromiseIsHandledToTrue(promise: PromiseLike<unknown>): void {
  promise.then(undefined, (e) => {
    if (e && e instanceof AssertionError) {
      queueMicrotask(() => {
        throw e;
      });
    }
  });
}

function setUpReadableByteStreamController(
  stream: ReadableStreamImpl,
  controller: ReadableByteStreamControllerImpl,
  startAlgorithm: StartAlgorithm,
  pullAlgorithm: PullAlgorithm,
  cancelAlgorithm: CancelAlgorithm,
  highWaterMark: number,
  autoAllocateChunkSize: number | undefined
): void {
  assert(stream[sym.readableStreamController] === undefined);
  if (autoAllocateChunkSize !== undefined) {
    assert(Number.isInteger(autoAllocateChunkSize));
    assert(autoAllocateChunkSize >= 0);
  }
  controller[sym.controlledReadableByteStream] = stream;
  controller[sym.pulling] = controller[sym.pullAgain] = false;
  controller[sym.byobRequest] = undefined;
  controller[sym.queue] = [];
  controller[sym.queueTotalSize] = 0;
  controller[sym.closeRequested] = controller[sym.started] = false;
  controller[sym.strategyHWM] = validateAndNormalizeHighWaterMark(
    highWaterMark
  );
  controller[sym.pullAlgorithm] = pullAlgorithm;
  controller[sym.cancelAlgorithm] = cancelAlgorithm;
  controller[sym.autoAllocateChunkSize] = autoAllocateChunkSize;
  // 3.13.26.12 Set controller.[[pendingPullIntos]] to a new empty List.
  stream[sym.readableStreamController] = controller;
  const startResult = startAlgorithm();
  const startPromise = Promise.resolve(startResult);
  setPromiseIsHandledToTrue(
    startPromise.then(
      () => {
        controller[sym.started] = true;
        assert(!controller[sym.pulling]);
        assert(!controller[sym.pullAgain]);
        readableByteStreamControllerCallPullIfNeeded(controller);
      },
      (r) => {
        readableByteStreamControllerError(controller, r);
      }
    )
  );
}

export function setUpReadableByteStreamControllerFromUnderlyingSource(
  stream: ReadableStreamImpl,
  underlyingByteSource: UnderlyingByteSource,
  highWaterMark: number
): void {
  assert(underlyingByteSource);
  const controller: ReadableByteStreamControllerImpl = Object.create(
    ReadableByteStreamControllerImpl.prototype
  );
  const startAlgorithm: StartAlgorithm = () => {
    return invokeOrNoop(underlyingByteSource, "start", controller);
  };
  const pullAlgorithm = createAlgorithmFromUnderlyingMethod(
    underlyingByteSource,
    "pull",
    0,
    controller
  );
  setFunctionName(pullAlgorithm, "[[pullAlgorithm]]");
  const cancelAlgorithm = createAlgorithmFromUnderlyingMethod(
    underlyingByteSource,
    "cancel",
    1
  );
  setFunctionName(cancelAlgorithm, "[[cancelAlgorithm]]");
  // 3.13.27.6 Let autoAllocateChunkSize be ? GetV(underlyingByteSource, "autoAllocateChunkSize").
  const autoAllocateChunkSize = undefined;
  setUpReadableByteStreamController(
    stream,
    controller,
    startAlgorithm,
    pullAlgorithm,
    cancelAlgorithm,
    highWaterMark,
    autoAllocateChunkSize
  );
}

function setUpReadableStreamDefaultController<T>(
  stream: ReadableStreamImpl<T>,
  controller: ReadableStreamDefaultControllerImpl<T>,
  startAlgorithm: StartAlgorithm,
  pullAlgorithm: PullAlgorithm,
  cancelAlgorithm: CancelAlgorithm,
  highWaterMark: number,
  sizeAlgorithm: SizeAlgorithm<T>
): void {
  assert(stream[sym.readableStreamController] === undefined);
  controller[sym.controlledReadableStream] = stream;
  controller[sym.queue] = [];
  controller[sym.queueTotalSize] = 0;
  controller[sym.started] = controller[sym.closeRequested] = controller[
    sym.pullAgain
  ] = controller[sym.pulling] = false;
  controller[sym.strategySizeAlgorithm] = sizeAlgorithm;
  controller[sym.strategyHWM] = highWaterMark;
  controller[sym.pullAlgorithm] = pullAlgorithm;
  controller[sym.cancelAlgorithm] = cancelAlgorithm;
  stream[sym.readableStreamController] = controller;
  const startResult = startAlgorithm();
  const startPromise = Promise.resolve(startResult);
  setPromiseIsHandledToTrue(
    startPromise.then(
      () => {
        controller[sym.started] = true;
        assert(controller[sym.pulling] === false);
        assert(controller[sym.pullAgain] === false);
        readableStreamDefaultControllerCallPullIfNeeded(controller);
      },
      (r) => {
        readableStreamDefaultControllerError(controller, r);
      }
    )
  );
}

export function setUpReadableStreamDefaultControllerFromUnderlyingSource<T>(
  stream: ReadableStreamImpl<T>,
  underlyingSource: UnderlyingSource<T>,
  highWaterMark: number,
  sizeAlgorithm: SizeAlgorithm<T>
): void {
  assert(underlyingSource);
  const controller: ReadableStreamDefaultControllerImpl<T> = Object.create(
    ReadableStreamDefaultControllerImpl.prototype
  );
  const startAlgorithm: StartAlgorithm = (): void | PromiseLike<void> =>
    invokeOrNoop(underlyingSource, "start", controller);
  const pullAlgorithm: PullAlgorithm = createAlgorithmFromUnderlyingMethod(
    underlyingSource,
    "pull",
    0,
    controller
  );
  setFunctionName(pullAlgorithm, "[[pullAlgorithm]]");
  const cancelAlgorithm: CancelAlgorithm = createAlgorithmFromUnderlyingMethod(
    underlyingSource,
    "cancel",
    1
  );
  setFunctionName(cancelAlgorithm, "[[cancelAlgorithm]]");
  setUpReadableStreamDefaultController(
    stream,
    controller,
    startAlgorithm,
    pullAlgorithm,
    cancelAlgorithm,
    highWaterMark,
    sizeAlgorithm
  );
}

function setUpTransformStreamDefaultController<I, O>(
  stream: TransformStreamImpl<I, O>,
  controller: TransformStreamDefaultControllerImpl<I, O>,
  transformAlgorithm: TransformAlgorithm<I>,
  flushAlgorithm: FlushAlgorithm
): void {
  assert(isTransformStream(stream));
  assert(stream[sym.transformStreamController] === undefined);
  controller[sym.controlledTransformStream] = stream;
  stream[sym.transformStreamController] = controller;
  controller[sym.transformAlgorithm] = transformAlgorithm;
  controller[sym.flushAlgorithm] = flushAlgorithm;
}

export function setUpTransformStreamDefaultControllerFromTransformer<I, O>(
  stream: TransformStreamImpl<I, O>,
  transformer: Transformer<I, O>
): void {
  assert(transformer);
  const controller = Object.create(
    TransformStreamDefaultControllerImpl.prototype
  ) as TransformStreamDefaultControllerImpl<I, O>;
  let transformAlgorithm: TransformAlgorithm<I> = (chunk) => {
    try {
      transformStreamDefaultControllerEnqueue(
        controller,
        // it defaults to no tranformation, so I is assumed to be O
        (chunk as unknown) as O
      );
    } catch (e) {
      return Promise.reject(e);
    }
    return Promise.resolve();
  };
  const transformMethod = transformer.transform;
  if (transformMethod) {
    if (typeof transformMethod !== "function") {
      throw new TypeError("tranformer.transform must be callable.");
    }
    transformAlgorithm = async (chunk): Promise<void> =>
      call(transformMethod, transformer, [chunk, controller]);
  }
  const flushAlgorithm = createAlgorithmFromUnderlyingMethod(
    transformer,
    "flush",
    0,
    controller
  );
  setUpTransformStreamDefaultController(
    stream,
    controller,
    transformAlgorithm,
    flushAlgorithm
  );
}

function setUpWritableStreamDefaultController<W>(
  stream: WritableStreamImpl<W>,
  controller: WritableStreamDefaultControllerImpl<W>,
  startAlgorithm: StartAlgorithm,
  writeAlgorithm: WriteAlgorithm<W>,
  closeAlgorithm: CloseAlgorithm,
  abortAlgorithm: AbortAlgorithm,
  highWaterMark: number,
  sizeAlgorithm: SizeAlgorithm<W>
): void {
  assert(isWritableStream(stream));
  assert(stream[sym.writableStreamController] === undefined);
  controller[sym.controlledWritableStream] = stream;
  stream[sym.writableStreamController] = controller;
  controller[sym.queue] = [];
  controller[sym.queueTotalSize] = 0;
  controller[sym.started] = false;
  controller[sym.strategySizeAlgorithm] = sizeAlgorithm;
  controller[sym.strategyHWM] = highWaterMark;
  controller[sym.writeAlgorithm] = writeAlgorithm;
  controller[sym.closeAlgorithm] = closeAlgorithm;
  controller[sym.abortAlgorithm] = abortAlgorithm;
  const backpressure = writableStreamDefaultControllerGetBackpressure(
    controller
  );
  writableStreamUpdateBackpressure(stream, backpressure);
  const startResult = startAlgorithm();
  const startPromise = Promise.resolve(startResult);
  setPromiseIsHandledToTrue(
    startPromise.then(
      () => {
        assert(
          stream[sym.state] === "writable" || stream[sym.state] === "erroring"
        );
        controller[sym.started] = true;
        writableStreamDefaultControllerAdvanceQueueIfNeeded(controller);
      },
      (r) => {
        assert(
          stream[sym.state] === "writable" || stream[sym.state] === "erroring"
        );
        controller[sym.started] = true;
        writableStreamDealWithRejection(stream, r);
      }
    )
  );
}

export function setUpWritableStreamDefaultControllerFromUnderlyingSink<W>(
  stream: WritableStreamImpl<W>,
  underlyingSink: UnderlyingSink<W>,
  highWaterMark: number,
  sizeAlgorithm: SizeAlgorithm<W>
): void {
  assert(underlyingSink);
  const controller = Object.create(
    WritableStreamDefaultControllerImpl.prototype
  );
  const startAlgorithm = (): void | PromiseLike<void> => {
    return invokeOrNoop(underlyingSink, "start", controller);
  };
  const writeAlgorithm = createAlgorithmFromUnderlyingMethod(
    underlyingSink,
    "write",
    1,
    controller
  );
  setFunctionName(writeAlgorithm, "[[writeAlgorithm]]");
  const closeAlgorithm = createAlgorithmFromUnderlyingMethod(
    underlyingSink,
    "close",
    0
  );
  setFunctionName(closeAlgorithm, "[[closeAlgorithm]]");
  const abortAlgorithm = createAlgorithmFromUnderlyingMethod(
    underlyingSink,
    "abort",
    1
  );
  setFunctionName(abortAlgorithm, "[[abortAlgorithm]]");
  setUpWritableStreamDefaultController(
    stream,
    controller,
    startAlgorithm,
    writeAlgorithm,
    closeAlgorithm,
    abortAlgorithm,
    highWaterMark,
    sizeAlgorithm
  );
}

function transformStreamDefaultControllerClearAlgorithms<I, O>(
  controller: TransformStreamDefaultControllerImpl<I, O>
): void {
  (controller as any)[sym.transformAlgorithm] = undefined;
  (controller as any)[sym.flushAlgorithm] = undefined;
}

export function transformStreamDefaultControllerEnqueue<I, O>(
  controller: TransformStreamDefaultControllerImpl<I, O>,
  chunk: O
): void {
  const stream = controller[sym.controlledTransformStream];
  const readableController = stream[sym.readable][
    sym.readableStreamController
  ] as ReadableStreamDefaultControllerImpl<O>;
  if (!readableStreamDefaultControllerCanCloseOrEnqueue(readableController)) {
    throw new TypeError(
      "TransformStream's readable controller cannot be closed or enqueued."
    );
  }
  try {
    readableStreamDefaultControllerEnqueue(readableController, chunk);
  } catch (e) {
    transformStreamErrorWritableAndUnblockWrite(stream, e);
    throw stream[sym.readable][sym.storedError];
  }
  const backpressure = readableStreamDefaultControllerHasBackpressure(
    readableController
  );
  if (backpressure) {
    transformStreamSetBackpressure(stream, true);
  }
}

export function transformStreamDefaultControllerError<I, O>(
  controller: TransformStreamDefaultControllerImpl<I, O>,
  e: any
): void {
  transformStreamError(controller[sym.controlledTransformStream], e);
}

function transformStreamDefaultControllerPerformTransform<I, O>(
  controller: TransformStreamDefaultControllerImpl<I, O>,
  chunk: I
): Promise<void> {
  const transformPromise = controller[sym.transformAlgorithm](chunk);
  return transformPromise.then(undefined, (r) => {
    transformStreamError(controller[sym.controlledTransformStream], r);
    throw r;
  });
}

function transformStreamDefaultSinkAbortAlgorithm<I, O>(
  stream: TransformStreamImpl<I, O>,
  reason: any
): Promise<void> {
  transformStreamError(stream, reason);
  return Promise.resolve(undefined);
}

function transformStreamDefaultSinkCloseAlgorithm<I, O>(
  stream: TransformStreamImpl<I, O>
): Promise<void> {
  const readable = stream[sym.readable];
  const controller = stream[sym.transformStreamController];
  const flushPromise = controller[sym.flushAlgorithm]();
  transformStreamDefaultControllerClearAlgorithms(controller);
  return flushPromise.then(
    () => {
      if (readable[sym.state] === "errored") {
        throw readable[sym.storedError];
      }
      const readableController = readable[
        sym.readableStreamController
      ] as ReadableStreamDefaultControllerImpl<O>;
      if (
        readableStreamDefaultControllerCanCloseOrEnqueue(readableController)
      ) {
        readableStreamDefaultControllerClose(readableController);
      }
    },
    (r) => {
      transformStreamError(stream, r);
      throw readable[sym.storedError];
    }
  );
}

function transformStreamDefaultSinkWriteAlgorithm<I, O>(
  stream: TransformStreamImpl<I, O>,
  chunk: I
): Promise<void> {
  assert(stream[sym.writable][sym.state] === "writable");
  const controller = stream[sym.transformStreamController];
  if (stream[sym.backpressure]) {
    const backpressureChangePromise = stream[sym.backpressureChangePromise];
    assert(backpressureChangePromise);
    return backpressureChangePromise.promise.then(() => {
      const writable = stream[sym.writable];
      const state = writable[sym.state];
      if (state === "erroring") {
        throw writable[sym.storedError];
      }
      assert(state === "writable");
      return transformStreamDefaultControllerPerformTransform(
        controller,
        chunk
      );
    });
  }
  return transformStreamDefaultControllerPerformTransform(controller, chunk);
}

function transformStreamDefaultSourcePullAlgorithm<I, O>(
  stream: TransformStreamImpl<I, O>
): Promise<void> {
  assert(stream[sym.backpressure] === true);
  assert(stream[sym.backpressureChangePromise] !== undefined);
  transformStreamSetBackpressure(stream, false);
  return stream[sym.backpressureChangePromise]!.promise;
}

function transformStreamError<I, O>(
  stream: TransformStreamImpl<I, O>,
  e: any
): void {
  readableStreamDefaultControllerError(
    stream[sym.readable][
      sym.readableStreamController
    ] as ReadableStreamDefaultControllerImpl<O>,
    e
  );
  transformStreamErrorWritableAndUnblockWrite(stream, e);
}

export function transformStreamDefaultControllerTerminate<I, O>(
  controller: TransformStreamDefaultControllerImpl<I, O>
): void {
  const stream = controller[sym.controlledTransformStream];
  const readableController = stream[sym.readable][
    sym.readableStreamController
  ] as ReadableStreamDefaultControllerImpl<O>;
  readableStreamDefaultControllerClose(readableController);
  const error = new TypeError("TransformStream is closed.");
  transformStreamErrorWritableAndUnblockWrite(stream, error);
}

function transformStreamErrorWritableAndUnblockWrite<I, O>(
  stream: TransformStreamImpl<I, O>,
  e: any
): void {
  transformStreamDefaultControllerClearAlgorithms(
    stream[sym.transformStreamController]
  );
  writableStreamDefaultControllerErrorIfNeeded(
    stream[sym.writable][sym.writableStreamController]!,
    e
  );
  if (stream[sym.backpressure]) {
    transformStreamSetBackpressure(stream, false);
  }
}

function transformStreamSetBackpressure<I, O>(
  stream: TransformStreamImpl<I, O>,
  backpressure: boolean
): void {
  assert(stream[sym.backpressure] !== backpressure);
  if (stream[sym.backpressureChangePromise] !== undefined) {
    stream[sym.backpressureChangePromise]!.resolve!(undefined);
  }
  stream[sym.backpressureChangePromise] = getDeferred<void>();
  stream[sym.backpressure] = backpressure;
}

function transferArrayBuffer(buffer: ArrayBuffer): ArrayBuffer {
  assert(!isDetachedBuffer(buffer));
  const transferredIshVersion = buffer.slice(0);

  Object.defineProperty(buffer, "byteLength", {
    get(): number {
      return 0;
    },
  });
  (buffer as any)[sym.isFakeDetached] = true;

  return transferredIshVersion;
}

export function validateAndNormalizeHighWaterMark(
  highWaterMark: number
): number {
  highWaterMark = Number(highWaterMark);
  if (Number.isNaN(highWaterMark) || highWaterMark < 0) {
    throw new RangeError(
      `highWaterMark must be a positive number or Infinity.  Received: ${highWaterMark}.`
    );
  }
  return highWaterMark;
}

export function writableStreamAbort<W>(
  stream: WritableStreamImpl<W>,
  reason: any
): Promise<void> {
  const state = stream[sym.state];
  if (state === "closed" || state === "errored") {
    return Promise.resolve(undefined);
  }
  if (stream[sym.pendingAbortRequest]) {
    return stream[sym.pendingAbortRequest]!.promise.promise;
  }
  assert(state === "writable" || state === "erroring");
  let wasAlreadyErroring = false;
  if (state === "erroring") {
    wasAlreadyErroring = true;
    reason = undefined;
  }
  const promise = getDeferred<void>();
  stream[sym.pendingAbortRequest] = { promise, reason, wasAlreadyErroring };

  if (wasAlreadyErroring === false) {
    writableStreamStartErroring(stream, reason);
  }
  return promise.promise;
}

function writableStreamAddWriteRequest<W>(
  stream: WritableStreamImpl<W>
): Promise<void> {
  assert(isWritableStream(stream));
  assert(stream[sym.state] === "writable");
  const promise = getDeferred<void>();
  stream[sym.writeRequests].push(promise);
  return promise.promise;
}

export function writableStreamClose<W>(
  stream: WritableStreamImpl<W>
): Promise<void> {
  const state = stream[sym.state];
  if (state === "closed" || state === "errored") {
    return Promise.reject(
      new TypeError("Cannot close an already closed or errored WritableStream.")
    );
  }
  assert(!writableStreamCloseQueuedOrInFlight(stream));
  const promise = getDeferred<void>();
  stream[sym.closeRequest] = promise;
  const writer = stream[sym.writer];
  if (writer && stream[sym.backpressure] && state === "writable") {
    writer[sym.readyPromise].resolve!();
    writer[sym.readyPromise].resolve = undefined;
    writer[sym.readyPromise].reject = undefined;
  }
  writableStreamDefaultControllerClose(stream[sym.writableStreamController]!);
  return promise.promise;
}

export function writableStreamCloseQueuedOrInFlight<W>(
  stream: WritableStreamImpl<W>
): boolean {
  if (
    stream[sym.closeRequest] === undefined &&
    stream[sym.inFlightCloseRequest] === undefined
  ) {
    return false;
  }
  return true;
}

function writableStreamDealWithRejection<W>(
  stream: WritableStreamImpl<W>,
  error: any
): void {
  const state = stream[sym.state];
  if (state === "writable") {
    writableStreamStartErroring(stream, error);
    return;
  }
  assert(state === "erroring");
  writableStreamFinishErroring(stream);
}

function writableStreamDefaultControllerAdvanceQueueIfNeeded<W>(
  controller: WritableStreamDefaultControllerImpl<W>
): void {
  const stream = controller[sym.controlledWritableStream];
  if (!controller[sym.started]) {
    return;
  }
  if (stream[sym.inFlightWriteRequest]) {
    return;
  }
  const state = stream[sym.state];
  assert(state !== "closed" && state !== "errored");
  if (state === "erroring") {
    writableStreamFinishErroring(stream);
    return;
  }
  if (!controller[sym.queue].length) {
    return;
  }
  const writeRecord = peekQueueValue(controller);
  if (writeRecord === "close") {
    writableStreamDefaultControllerProcessClose(controller);
  } else {
    writableStreamDefaultControllerProcessWrite(controller, writeRecord.chunk);
  }
}

export function writableStreamDefaultControllerClearAlgorithms<W>(
  controller: WritableStreamDefaultControllerImpl<W>
): void {
  (controller as any)[sym.writeAlgorithm] = undefined;
  (controller as any)[sym.closeAlgorithm] = undefined;
  (controller as any)[sym.abortAlgorithm] = undefined;
  (controller as any)[sym.strategySizeAlgorithm] = undefined;
}

function writableStreamDefaultControllerClose<W>(
  controller: WritableStreamDefaultControllerImpl<W>
): void {
  enqueueValueWithSize(controller, "close", 0);
  writableStreamDefaultControllerAdvanceQueueIfNeeded(controller);
}

export function writableStreamDefaultControllerError<W>(
  controller: WritableStreamDefaultControllerImpl<W>,
  error: any
): void {
  const stream = controller[sym.controlledWritableStream];
  assert(stream[sym.state] === "writable");
  writableStreamDefaultControllerClearAlgorithms(controller);
  writableStreamStartErroring(stream, error);
}

function writableStreamDefaultControllerErrorIfNeeded<W>(
  controller: WritableStreamDefaultControllerImpl<W>,
  error: any
): void {
  if (controller[sym.controlledWritableStream][sym.state] === "writable") {
    writableStreamDefaultControllerError(controller, error);
  }
}

function writableStreamDefaultControllerGetBackpressure<W>(
  controller: WritableStreamDefaultControllerImpl<W>
): boolean {
  const desiredSize = writableStreamDefaultControllerGetDesiredSize(controller);
  return desiredSize <= 0;
}

function writableStreamDefaultControllerGetChunkSize<W>(
  controller: WritableStreamDefaultControllerImpl<W>,
  chunk: W
): number {
  let returnValue: number;
  try {
    returnValue = controller[sym.strategySizeAlgorithm](chunk);
  } catch (e) {
    writableStreamDefaultControllerErrorIfNeeded(controller, e);
    return 1;
  }
  return returnValue;
}

function writableStreamDefaultControllerGetDesiredSize<W>(
  controller: WritableStreamDefaultControllerImpl<W>
): number {
  return controller[sym.strategyHWM] - controller[sym.queueTotalSize];
}

function writableStreamDefaultControllerProcessClose<W>(
  controller: WritableStreamDefaultControllerImpl<W>
): void {
  const stream = controller[sym.controlledWritableStream];
  writableStreamMarkCloseRequestInFlight(stream);
  dequeueValue(controller);
  assert(controller[sym.queue].length === 0);
  const sinkClosePromise = controller[sym.closeAlgorithm]();
  writableStreamDefaultControllerClearAlgorithms(controller);
  setPromiseIsHandledToTrue(
    sinkClosePromise.then(
      () => {
        writableStreamFinishInFlightClose(stream);
      },
      (reason) => {
        writableStreamFinishInFlightCloseWithError(stream, reason);
      }
    )
  );
}

function writableStreamDefaultControllerProcessWrite<W>(
  controller: WritableStreamDefaultControllerImpl<W>,
  chunk: W
): void {
  const stream = controller[sym.controlledWritableStream];
  writableStreamMarkFirstWriteRequestInFlight(stream);
  const sinkWritePromise = controller[sym.writeAlgorithm](chunk);
  setPromiseIsHandledToTrue(
    sinkWritePromise.then(
      () => {
        writableStreamFinishInFlightWrite(stream);
        const state = stream[sym.state];
        assert(state === "writable" || state === "erroring");
        dequeueValue(controller);
        if (
          !writableStreamCloseQueuedOrInFlight(stream) &&
          state === "writable"
        ) {
          const backpressure = writableStreamDefaultControllerGetBackpressure(
            controller
          );
          writableStreamUpdateBackpressure(stream, backpressure);
        }
        writableStreamDefaultControllerAdvanceQueueIfNeeded(controller);
      },
      (reason) => {
        if (stream[sym.state] === "writable") {
          writableStreamDefaultControllerClearAlgorithms(controller);
        }
        writableStreamFinishInFlightWriteWithError(stream, reason);
      }
    )
  );
}

function writableStreamDefaultControllerWrite<W>(
  controller: WritableStreamDefaultControllerImpl<W>,
  chunk: W,
  chunkSize: number
): void {
  const writeRecord = { chunk };
  try {
    enqueueValueWithSize(controller, writeRecord, chunkSize);
  } catch (e) {
    writableStreamDefaultControllerErrorIfNeeded(controller, e);
    return;
  }
  const stream = controller[sym.controlledWritableStream];
  if (
    !writableStreamCloseQueuedOrInFlight(stream) &&
    stream[sym.state] === "writable"
  ) {
    const backpressure = writableStreamDefaultControllerGetBackpressure(
      controller
    );
    writableStreamUpdateBackpressure(stream, backpressure);
  }
  writableStreamDefaultControllerAdvanceQueueIfNeeded(controller);
}

export function writableStreamDefaultWriterAbort<W>(
  writer: WritableStreamDefaultWriterImpl<W>,
  reason: any
): Promise<void> {
  const stream = writer[sym.ownerWritableStream];
  assert(stream);
  return writableStreamAbort(stream, reason);
}

export function writableStreamDefaultWriterClose<W>(
  writer: WritableStreamDefaultWriterImpl<W>
): Promise<void> {
  const stream = writer[sym.ownerWritableStream];
  assert(stream);
  return writableStreamClose(stream);
}

function writableStreamDefaultWriterCloseWithErrorPropagation<W>(
  writer: WritableStreamDefaultWriterImpl<W>
): Promise<void> {
  const stream = writer[sym.ownerWritableStream];
  assert(stream);
  const state = stream[sym.state];
  if (writableStreamCloseQueuedOrInFlight(stream) || state === "closed") {
    return Promise.resolve();
  }
  if (state === "errored") {
    return Promise.reject(stream[sym.storedError]);
  }
  assert(state === "writable" || state === "erroring");
  return writableStreamDefaultWriterClose(writer);
}

function writableStreamDefaultWriterEnsureClosePromiseRejected<W>(
  writer: WritableStreamDefaultWriterImpl<W>,
  error: any
): void {
  if (writer[sym.closedPromise].reject) {
    writer[sym.closedPromise].reject!(error);
  } else {
    writer[sym.closedPromise] = {
      promise: Promise.reject(error),
    };
  }
  setPromiseIsHandledToTrue(writer[sym.closedPromise].promise);
}

function writableStreamDefaultWriterEnsureReadyPromiseRejected<W>(
  writer: WritableStreamDefaultWriterImpl<W>,
  error: any
): void {
  if (writer[sym.readyPromise].reject) {
    writer[sym.readyPromise].reject!(error);
    writer[sym.readyPromise].reject = undefined;
    writer[sym.readyPromise].resolve = undefined;
  } else {
    writer[sym.readyPromise] = {
      promise: Promise.reject(error),
    };
  }
  setPromiseIsHandledToTrue(writer[sym.readyPromise].promise);
}

export function writableStreamDefaultWriterWrite<W>(
  writer: WritableStreamDefaultWriterImpl<W>,
  chunk: W
): Promise<void> {
  const stream = writer[sym.ownerWritableStream];
  assert(stream);
  const controller = stream[sym.writableStreamController];
  assert(controller);
  const chunkSize = writableStreamDefaultControllerGetChunkSize(
    controller,
    chunk
  );
  if (stream !== writer[sym.ownerWritableStream]) {
    return Promise.reject("Writer has incorrect WritableStream.");
  }
  const state = stream[sym.state];
  if (state === "errored") {
    return Promise.reject(stream[sym.storedError]);
  }
  if (writableStreamCloseQueuedOrInFlight(stream) || state === "closed") {
    return Promise.reject(new TypeError("The stream is closed or closing."));
  }
  if (state === "erroring") {
    return Promise.reject(stream[sym.storedError]);
  }
  assert(state === "writable");
  const promise = writableStreamAddWriteRequest(stream);
  writableStreamDefaultControllerWrite(controller, chunk, chunkSize);
  return promise;
}

export function writableStreamDefaultWriterGetDesiredSize<W>(
  writer: WritableStreamDefaultWriterImpl<W>
): number | null {
  const stream = writer[sym.ownerWritableStream];
  const state = stream[sym.state];
  if (state === "errored" || state === "erroring") {
    return null;
  }
  if (state === "closed") {
    return 0;
  }
  return writableStreamDefaultControllerGetDesiredSize(
    stream[sym.writableStreamController]!
  );
}

export function writableStreamDefaultWriterRelease<W>(
  writer: WritableStreamDefaultWriterImpl<W>
): void {
  const stream = writer[sym.ownerWritableStream];
  assert(stream);
  assert(stream[sym.writer] === writer);
  const releasedError = new TypeError(
    "Writer was released and can no longer be used to monitor the stream's closedness."
  );
  writableStreamDefaultWriterEnsureReadyPromiseRejected(writer, releasedError);
  writableStreamDefaultWriterEnsureClosePromiseRejected(writer, releasedError);
  stream[sym.writer] = undefined;
  (writer as any)[sym.ownerWritableStream] = undefined;
}

function writableStreamFinishErroring<W>(stream: WritableStreamImpl<W>): void {
  assert(stream[sym.state] === "erroring");
  assert(!writableStreamHasOperationMarkedInFlight(stream));
  stream[sym.state] = "errored";
  stream[sym.writableStreamController]![sym.errorSteps]();
  const storedError = stream[sym.storedError];
  for (const writeRequest of stream[sym.writeRequests]) {
    assert(writeRequest.reject);
    writeRequest.reject(storedError);
  }
  stream[sym.writeRequests] = [];
  if (!stream[sym.pendingAbortRequest]) {
    writableStreamRejectCloseAndClosedPromiseIfNeeded(stream);
    return;
  }
  const abortRequest = stream[sym.pendingAbortRequest];
  assert(abortRequest);
  stream[sym.pendingAbortRequest] = undefined;
  if (abortRequest.wasAlreadyErroring) {
    assert(abortRequest.promise.reject);
    abortRequest.promise.reject(storedError);
    writableStreamRejectCloseAndClosedPromiseIfNeeded(stream);
    return;
  }
  const promise = stream[sym.writableStreamController]![sym.abortSteps](
    abortRequest.reason
  );
  setPromiseIsHandledToTrue(
    promise.then(
      () => {
        assert(abortRequest.promise.resolve);
        abortRequest.promise.resolve();
        writableStreamRejectCloseAndClosedPromiseIfNeeded(stream);
      },
      (reason) => {
        assert(abortRequest.promise.reject);
        abortRequest.promise.reject(reason);
        writableStreamRejectCloseAndClosedPromiseIfNeeded(stream);
      }
    )
  );
}

function writableStreamFinishInFlightClose<W>(
  stream: WritableStreamImpl<W>
): void {
  assert(stream[sym.inFlightCloseRequest]);
  stream[sym.inFlightCloseRequest]?.resolve!();
  stream[sym.inFlightCloseRequest] = undefined;
  const state = stream[sym.state];
  assert(state === "writable" || state === "erroring");
  if (state === "erroring") {
    stream[sym.storedError] = undefined;
    if (stream[sym.pendingAbortRequest]) {
      stream[sym.pendingAbortRequest]!.promise.resolve!();
      stream[sym.pendingAbortRequest] = undefined;
    }
  }
  stream[sym.state] = "closed";
  const writer = stream[sym.writer];
  if (writer) {
    writer[sym.closedPromise].resolve!();
  }
  assert(stream[sym.pendingAbortRequest] === undefined);
  assert(stream[sym.storedError] === undefined);
}

function writableStreamFinishInFlightCloseWithError<W>(
  stream: WritableStreamImpl<W>,
  error: any
): void {
  assert(stream[sym.inFlightCloseRequest]);
  stream[sym.inFlightCloseRequest]?.reject!(error);
  stream[sym.inFlightCloseRequest] = undefined;
  assert(stream[sym.state] === "writable" || stream[sym.state] === "erroring");
  if (stream[sym.pendingAbortRequest]) {
    stream[sym.pendingAbortRequest]?.promise.reject!(error);
    stream[sym.pendingAbortRequest] = undefined;
  }
  writableStreamDealWithRejection(stream, error);
}

function writableStreamFinishInFlightWrite<W>(
  stream: WritableStreamImpl<W>
): void {
  assert(stream[sym.inFlightWriteRequest]);
  stream[sym.inFlightWriteRequest]!.resolve();
  stream[sym.inFlightWriteRequest] = undefined;
}

function writableStreamFinishInFlightWriteWithError<W>(
  stream: WritableStreamImpl<W>,
  error: any
): void {
  assert(stream[sym.inFlightWriteRequest]);
  stream[sym.inFlightWriteRequest]!.reject!(error);
  stream[sym.inFlightWriteRequest] = undefined;
  assert(stream[sym.state] === "writable" || stream[sym.state] === "erroring");
  writableStreamDealWithRejection(stream, error);
}

function writableStreamHasOperationMarkedInFlight<W>(
  stream: WritableStreamImpl<W>
): boolean {
  if (
    stream[sym.inFlightWriteRequest] === undefined &&
    stream[sym.inFlightCloseRequest] === undefined
  ) {
    return false;
  }
  return true;
}

function writableStreamMarkCloseRequestInFlight<W>(
  stream: WritableStreamImpl<W>
): void {
  assert(stream[sym.inFlightCloseRequest] === undefined);
  assert(stream[sym.closeRequest] !== undefined);
  stream[sym.inFlightCloseRequest] = stream[sym.closeRequest];
  stream[sym.closeRequest] = undefined;
}

function writableStreamMarkFirstWriteRequestInFlight<W>(
  stream: WritableStreamImpl<W>
): void {
  assert(stream[sym.inFlightWriteRequest] === undefined);
  assert(stream[sym.writeRequests].length);
  const writeRequest = stream[sym.writeRequests].shift();
  stream[sym.inFlightWriteRequest] = writeRequest;
}

function writableStreamRejectCloseAndClosedPromiseIfNeeded<W>(
  stream: WritableStreamImpl<W>
): void {
  assert(stream[sym.state] === "errored");
  if (stream[sym.closeRequest]) {
    assert(stream[sym.inFlightCloseRequest] === undefined);
    stream[sym.closeRequest]!.reject!(stream[sym.storedError]);
    stream[sym.closeRequest] = undefined;
  }
  const writer = stream[sym.writer];
  if (writer) {
    writer[sym.closedPromise].reject!(stream[sym.storedError]);
    setPromiseIsHandledToTrue(writer[sym.closedPromise].promise);
  }
}

function writableStreamStartErroring<W>(
  stream: WritableStreamImpl<W>,
  reason: any
): void {
  assert(stream[sym.storedError] === undefined);
  assert(stream[sym.state] === "writable");
  const controller = stream[sym.writableStreamController];
  assert(controller);
  stream[sym.state] = "erroring";
  stream[sym.storedError] = reason;
  const writer = stream[sym.writer];
  if (writer) {
    writableStreamDefaultWriterEnsureReadyPromiseRejected(writer, reason);
  }
  if (
    !writableStreamHasOperationMarkedInFlight(stream) &&
    controller[sym.started]
  ) {
    writableStreamFinishErroring(stream);
  }
}

function writableStreamUpdateBackpressure<W>(
  stream: WritableStreamImpl<W>,
  backpressure: boolean
): void {
  assert(stream[sym.state] === "writable");
  assert(!writableStreamCloseQueuedOrInFlight(stream));
  const writer = stream[sym.writer];
  if (writer && backpressure !== stream[sym.backpressure]) {
    if (backpressure) {
      writer[sym.readyPromise] = getDeferred();
    } else {
      assert(backpressure === false);
      writer[sym.readyPromise].resolve!();
      writer[sym.readyPromise].resolve = undefined;
      writer[sym.readyPromise].reject = undefined;
    }
  }
  stream[sym.backpressure] = backpressure;
}

/* eslint-enable */
