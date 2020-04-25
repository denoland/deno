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
import { cloneValue } from "../util.ts";
import { assert } from "../../util.ts";

export interface BufferQueueItem extends Pair<ArrayBuffer | SharedArrayBuffer> {
  offset: number;
}
export type CancelAlgorithm = (reason?: any) => PromiseLike<void>;
type Container<R = any> = {
  [sym.queue]: Array<Pair<R> | BufferQueueItem>;
  [sym.queueTotalSize]: number;
};
export type Pair<R> = { value: R; size: number };
export type PullAlgorithm = () => PromiseLike<void>;
export type SizeAlgorithm<T> = (chunk: T) => number;
export type StartAlgorithm = () => void | PromiseLike<void>;
export interface Deferred<T> {
  promise: Promise<T>;
  resolve?: (value?: T | PromiseLike<T>) => void;
  reject?: (reason?: any) => void;
  settled: boolean;
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

function createAlgorithmFromUnderlyingMethod<
  O extends UnderlyingByteSource | UnderlyingSource,
  P extends keyof O
>(
  underlyingObject: O,
  methodName: P,
  algoArgCount: 0,
  ...extraArgs: any[]
): () => Promise<void>;
function createAlgorithmFromUnderlyingMethod<
  O extends UnderlyingByteSource | UnderlyingSource,
  P extends keyof O
>(
  underlyingObject: O,
  methodName: P,
  algoArgCount: 1,
  ...extraArgs: any[]
): (arg: any) => Promise<void>;
function createAlgorithmFromUnderlyingMethod<
  O extends UnderlyingByteSource | UnderlyingSource,
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
        method.call(underlyingObject, ...extraArgs);
    } else {
      return async (arg: any): Promise<void> => {
        const fullArgs = [arg, ...extraArgs];
        return method.call(underlyingObject, ...fullArgs);
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
  assert(isNonNegativeNumber(highWaterMark));
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
function getDeferred<T>(): Deferred<T> {
  let resolve = undefined;
  let reject = undefined;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject, settled: false };
}

export function initializeReadableStream(stream: ReadableStreamImpl): void {
  stream[sym.state] = "readable";
  stream[sym.reader] = stream[sym.storedError] = undefined;
  stream[sym.disturbed] = false;
}

function invokeOrNoop<O extends any, P extends keyof O>(
  o: O,
  p: P,
  ...args: Parameters<O[P]>
): ReturnType<O[P]> | undefined {
  assert(o);
  const method = o[p];
  if (!method) {
    return undefined;
  }
  return method.call(o, ...args);
}

function isCallable(value: unknown): value is (...args: any) => any {
  return typeof value === "function";
}

export function isDetachedBuffer(value: object): boolean {
  return sym.isFakeDetached in value;
}

function isFiniteNonNegativeNumber(v: unknown): v is number {
  if (!isNonNegativeNumber(v)) {
    return false;
  }
  if (v === Infinity) {
    return false;
  }
  return true;
}

function isNonNegativeNumber(v: unknown): v is number {
  if (typeof v !== "number") {
    return false;
  }
  if (v === NaN) {
    return false;
  }
  if (v < 0) {
    return false;
  }
  return true;
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

export function isUnderlyingByteSource(
  underlyingSource: UnderlyingByteSource | UnderlyingSource
): underlyingSource is UnderlyingByteSource {
  const { type } = underlyingSource;
  const typeString = String(type);
  return typeString === "bytes";
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
  pullPromise.then(
    () => {
      controller[sym.pulling] = false;
      if (controller[sym.pullAgain]) {
        controller[sym.pullAgain];
        readableByteStreamControllerCallPullIfNeeded(controller);
      }
    },
    (e) => {
      readableByteStreamControllerError(controller, e);
    }
  );
}

export function readableByteStreamControllerClearAlgorithms(
  controller: ReadableByteStreamControllerImpl
): void {
  delete controller[sym.pullAlgorithm];
  delete controller[sym.cancelAlgorithm];
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

export async function readableStreamCancel<T>(
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
  await stream[sym.readableStreamController]![sym.cancelSteps](reason);
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
  reader[sym.closedPromise].settled = true;
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
  delete controller[sym.pullAlgorithm];
  delete controller[sym.cancelAlgorithm];
  delete controller[sym.strategySizeAlgorithm];
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
      const { reject } = readRequest;
      assert(reject);
      reject(e);
    }
    reader[sym.readRequests] = [];
  }
  // 3.5.6.8 Otherwise, support BYOB Reader
  const { reject } = reader[sym.closedPromise];
  assert(reject);
  reject(e);
  reader[sym.closedPromise].settled = true;
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
    reader[sym.closedPromise] = {
      promise: Promise.resolve(),
      settled: true,
    };
  } else {
    assert(stream[sym.state] === "errored");
    reader[sym.closedPromise] = {
      promise: Promise.reject(stream[sym.storedError]),
      settled: true,
    };
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
  closedPromise.settled = true;
  delete reader[sym.ownerReadableStream][sym.reader];
  delete reader[sym.ownerReadableStream];
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
    readableStreamDefaultReaderRead(reader).then((result) => {
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
    });
    return Promise.resolve();
  };
  const cancel1Algorithm = (reason?: any): PromiseLike<void> => {
    canceled1 = true;
    reason1 = reason;
    if (canceled2) {
      const compositeReason = [reason1, reason2];
      const cancelResult = readableStreamCancel(stream, compositeReason);
      assert(cancelPromise.resolve);
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
      assert(cancelPromise.resolve);
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
  });
  return [branch1, branch2];
}

export function resetQueue<R>(container: Container<R>): void {
  assert(sym.queue in container && sym.queueTotalSize in container);
  container[sym.queue] = [];
  container[sym.queueTotalSize] = 0;
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
  const cancelAlgorithm = createAlgorithmFromUnderlyingMethod(
    underlyingByteSource,
    "cancel",
    1
  );
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
  const cancelAlgorithm: CancelAlgorithm = createAlgorithmFromUnderlyingMethod(
    underlyingSource,
    "cancel",
    1
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
  if (highWaterMark === NaN || highWaterMark < 0) {
    throw new RangeError(
      `highWaterMark must be a positive number or Infinity.  Received: ${highWaterMark}.`
    );
  }
  return highWaterMark;
}

/* eslint-enable */
