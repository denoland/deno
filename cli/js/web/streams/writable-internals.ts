// TODO reenable this code when we enable writableStreams and transport types
// // Forked from https://github.com/stardazed/sd-streams/tree/8928cf04b035fd02fb1340b7eb541c76be37e546
// // Copyright (c) 2018-Present by Arthur Langereis - @zenmumbler MIT

// /**
//  * streams/writable-internals - internal types and functions for writable streams
//  * Part of Stardazed
//  * (c) 2018-Present by Arthur Langereis - @zenmumbler
//  * https://github.com/stardazed/sd-streams
//  */

// /* eslint-disable @typescript-eslint/no-explicit-any */
// // TODO reenable this lint here

// import * as shared from "./shared-internals.ts";
// import * as q from "./queue-mixin.ts";

// import { QueuingStrategy, QueuingStrategySizeCallback } from "../dom_types.ts";

// export const backpressure_ = Symbol("backpressure_");
// export const closeRequest_ = Symbol("closeRequest_");
// export const inFlightWriteRequest_ = Symbol("inFlightWriteRequest_");
// export const inFlightCloseRequest_ = Symbol("inFlightCloseRequest_");
// export const pendingAbortRequest_ = Symbol("pendingAbortRequest_");
// export const writableStreamController_ = Symbol("writableStreamController_");
// export const writer_ = Symbol("writer_");
// export const writeRequests_ = Symbol("writeRequests_");

// export const abortAlgorithm_ = Symbol("abortAlgorithm_");
// export const closeAlgorithm_ = Symbol("closeAlgorithm_");
// export const controlledWritableStream_ = Symbol("controlledWritableStream_");
// export const started_ = Symbol("started_");
// export const strategyHWM_ = Symbol("strategyHWM_");
// export const strategySizeAlgorithm_ = Symbol("strategySizeAlgorithm_");
// export const writeAlgorithm_ = Symbol("writeAlgorithm_");

// export const ownerWritableStream_ = Symbol("ownerWritableStream_");
// export const closedPromise_ = Symbol("closedPromise_");
// export const readyPromise_ = Symbol("readyPromise_");

// export const errorSteps_ = Symbol("errorSteps_");
// export const abortSteps_ = Symbol("abortSteps_");

// export type StartFunction = (
//   controller: WritableStreamController
// ) => void | PromiseLike<void>;
// export type StartAlgorithm = () => Promise<void> | void;
// export type WriteFunction<InputType> = (
//   chunk: InputType,
//   controller: WritableStreamController
// ) => void | PromiseLike<void>;
// export type WriteAlgorithm<InputType> = (chunk: InputType) => Promise<void>;
// export type CloseAlgorithm = () => Promise<void>;
// export type AbortAlgorithm = (reason?: shared.ErrorResult) => Promise<void>;

// // ----

// export interface WritableStreamController {
//   error(e?: shared.ErrorResult): void;

//   [errorSteps_](): void;
//   [abortSteps_](reason: shared.ErrorResult): Promise<void>;
// }

// export interface WriteRecord<InputType> {
//   chunk: InputType;
// }

// export interface WritableStreamDefaultController<InputType>
//   extends WritableStreamController,
//     q.QueueContainer<WriteRecord<InputType> | "close"> {
//   [abortAlgorithm_]: AbortAlgorithm; // A promise - returning algorithm, taking one argument(the abort reason), which communicates a requested abort to the underlying sink
//   [closeAlgorithm_]: CloseAlgorithm; // A promise - returning algorithm which communicates a requested close to the underlying sink
//   [controlledWritableStream_]: WritableStream<InputType>; // The WritableStream instance controlled
//   [started_]: boolean; // A boolean flag indicating whether the underlying sink has finished starting
//   [strategyHWM_]: number; // A number supplied by the creator of the stream as part of the stream’s queuing strategy, indicating the point at which the stream will apply backpressure to its underlying sink
//   [strategySizeAlgorithm_]: QueuingStrategySizeCallback<InputType>; // An algorithm to calculate the size of enqueued chunks, as part of the stream’s queuing strategy
//   [writeAlgorithm_]: WriteAlgorithm<InputType>; // A promise-returning algorithm, taking one argument (the chunk to write), which writes data to the underlying sink
// }

// // ----

// export interface WritableStreamWriter<InputType> {
//   readonly closed: Promise<void>;
//   readonly desiredSize: number | null;
//   readonly ready: Promise<void>;

//   abort(reason: shared.ErrorResult): Promise<void>;
//   close(): Promise<void>;
//   releaseLock(): void;
//   write(chunk: InputType): Promise<void>;
// }

// export interface WritableStreamDefaultWriter<InputType>
//   extends WritableStreamWriter<InputType> {
//   [ownerWritableStream_]: WritableStream<InputType> | undefined;
//   [closedPromise_]: shared.ControlledPromise<void>;
//   [readyPromise_]: shared.ControlledPromise<void>;
// }

// // ----

// export type WritableStreamState =
//   | "writable"
//   | "closed"
//   | "erroring"
//   | "errored";

// export interface WritableStreamSink<InputType> {
//   start?: StartFunction;
//   write?: WriteFunction<InputType>;
//   close?(): void | PromiseLike<void>;
//   abort?(reason?: shared.ErrorResult): void;

//   type?: undefined; // unused, for future revisions
// }

// export interface AbortRequest {
//   reason: shared.ErrorResult;
//   wasAlreadyErroring: boolean;
//   promise: Promise<void>;
//   resolve(): void;
//   reject(error: shared.ErrorResult): void;
// }

// export declare class WritableStream<InputType> {
//   constructor(
//     underlyingSink?: WritableStreamSink<InputType>,
//     strategy?: QueuingStrategy<InputType>
//   );

//   readonly locked: boolean;
//   abort(reason?: shared.ErrorResult): Promise<void>;
//   getWriter(): WritableStreamWriter<InputType>;

//   [shared.state_]: WritableStreamState;
//   [backpressure_]: boolean;
//   [closeRequest_]: shared.ControlledPromise<void> | undefined;
//   [inFlightWriteRequest_]: shared.ControlledPromise<void> | undefined;
//   [inFlightCloseRequest_]: shared.ControlledPromise<void> | undefined;
//   [pendingAbortRequest_]: AbortRequest | undefined;
//   [shared.storedError_]: shared.ErrorResult;
//   [writableStreamController_]:
//     | WritableStreamDefaultController<InputType>
//     | undefined;
//   [writer_]: WritableStreamDefaultWriter<InputType> | undefined;
//   [writeRequests_]: Array<shared.ControlledPromise<void>>;
// }

// // ---- Stream

// export function initializeWritableStream<InputType>(
//   stream: WritableStream<InputType>
// ): void {
//   stream[shared.state_] = "writable";
//   stream[shared.storedError_] = undefined;
//   stream[writer_] = undefined;
//   stream[writableStreamController_] = undefined;
//   stream[inFlightWriteRequest_] = undefined;
//   stream[closeRequest_] = undefined;
//   stream[inFlightCloseRequest_] = undefined;
//   stream[pendingAbortRequest_] = undefined;
//   stream[writeRequests_] = [];
//   stream[backpressure_] = false;
// }

// export function isWritableStream(value: unknown): value is WritableStream<any> {
//   if (typeof value !== "object" || value === null) {
//     return false;
//   }
//   return writableStreamController_ in value;
// }

// export function isWritableStreamLocked<InputType>(
//   stream: WritableStream<InputType>
// ): boolean {
//   return stream[writer_] !== undefined;
// }

// export function writableStreamAbort<InputType>(
//   stream: WritableStream<InputType>,
//   reason: shared.ErrorResult
// ): Promise<void> {
//   const state = stream[shared.state_];
//   if (state === "closed" || state === "errored") {
//     return Promise.resolve(undefined);
//   }
//   let pending = stream[pendingAbortRequest_];
//   if (pending !== undefined) {
//     return pending.promise;
//   }
//   // Assert: state is "writable" or "erroring".
//   let wasAlreadyErroring = false;
//   if (state === "erroring") {
//     wasAlreadyErroring = true;
//     reason = undefined;
//   }

//   pending = {
//     reason,
//     wasAlreadyErroring
//   } as AbortRequest;
//   const promise = new Promise<void>((resolve, reject) => {
//     pending!.resolve = resolve;
//     pending!.reject = reject;
//   });
//   pending.promise = promise;
//   stream[pendingAbortRequest_] = pending;
//   if (!wasAlreadyErroring) {
//     writableStreamStartErroring(stream, reason);
//   }
//   return promise;
// }

// export function writableStreamAddWriteRequest<InputType>(
//   stream: WritableStream<InputType>
// ): Promise<void> {
//   // Assert: !IsWritableStreamLocked(stream) is true.
//   // Assert: stream.[[state]] is "writable".
//   const writePromise = shared.createControlledPromise<void>();
//   stream[writeRequests_].push(writePromise);
//   return writePromise.promise;
// }

// export function writableStreamDealWithRejection<InputType>(
//   stream: WritableStream<InputType>,
//   error: shared.ErrorResult
// ): void {
//   const state = stream[shared.state_];
//   if (state === "writable") {
//     writableStreamStartErroring(stream, error);
//     return;
//   }
//   // Assert: state is "erroring"
//   writableStreamFinishErroring(stream);
// }

// export function writableStreamStartErroring<InputType>(
//   stream: WritableStream<InputType>,
//   reason: shared.ErrorResult
// ): void {
//   // Assert: stream.[[storedError]] is undefined.
//   // Assert: stream.[[state]] is "writable".
//   const controller = stream[writableStreamController_]!;
//   // Assert: controller is not undefined.
//   stream[shared.state_] = "erroring";
//   stream[shared.storedError_] = reason;
//   const writer = stream[writer_];
//   if (writer !== undefined) {
//     writableStreamDefaultWriterEnsureReadyPromiseRejected(writer, reason);
//   }
//   if (
//     !writableStreamHasOperationMarkedInFlight(stream) &&
//     controller[started_]
//   ) {
//     writableStreamFinishErroring(stream);
//   }
// }

// export function writableStreamFinishErroring<InputType>(
//   stream: WritableStream<InputType>
// ): void {
//   // Assert: stream.[[state]] is "erroring".
//   // Assert: writableStreamHasOperationMarkedInFlight(stream) is false.
//   stream[shared.state_] = "errored";
//   const controller = stream[writableStreamController_]!;
//   controller[errorSteps_]();
//   const storedError = stream[shared.storedError_];
//   for (const writeRequest of stream[writeRequests_]) {
//     writeRequest.reject(storedError);
//   }
//   stream[writeRequests_] = [];

//   const abortRequest = stream[pendingAbortRequest_];
//   if (abortRequest === undefined) {
//     writableStreamRejectCloseAndClosedPromiseIfNeeded(stream);
//     return;
//   }
//   stream[pendingAbortRequest_] = undefined;
//   if (abortRequest.wasAlreadyErroring) {
//     abortRequest.reject(storedError);
//     writableStreamRejectCloseAndClosedPromiseIfNeeded(stream);
//     return;
//   }
//   const promise = controller[abortSteps_](abortRequest.reason);
//   promise.then(
//     _ => {
//       abortRequest.resolve();
//       writableStreamRejectCloseAndClosedPromiseIfNeeded(stream);
//     },
//     error => {
//       abortRequest.reject(error);
//       writableStreamRejectCloseAndClosedPromiseIfNeeded(stream);
//     }
//   );
// }

// export function writableStreamFinishInFlightWrite<InputType>(
//   stream: WritableStream<InputType>
// ): void {
//   // Assert: stream.[[inFlightWriteRequest]] is not undefined.
//   stream[inFlightWriteRequest_]!.resolve(undefined);
//   stream[inFlightWriteRequest_] = undefined;
// }

// export function writableStreamFinishInFlightWriteWithError<InputType>(
//   stream: WritableStream<InputType>,
//   error: shared.ErrorResult
// ): void {
//   // Assert: stream.[[inFlightWriteRequest]] is not undefined.
//   stream[inFlightWriteRequest_]!.reject(error);
//   stream[inFlightWriteRequest_] = undefined;
//   // Assert: stream.[[state]] is "writable" or "erroring".
//   writableStreamDealWithRejection(stream, error);
// }

// export function writableStreamFinishInFlightClose<InputType>(
//   stream: WritableStream<InputType>
// ): void {
//   // Assert: stream.[[inFlightCloseRequest]] is not undefined.
//   stream[inFlightCloseRequest_]!.resolve(undefined);
//   stream[inFlightCloseRequest_] = undefined;
//   const state = stream[shared.state_];
//   // Assert: stream.[[state]] is "writable" or "erroring".
//   if (state === "erroring") {
//     stream[shared.storedError_] = undefined;
//     if (stream[pendingAbortRequest_] !== undefined) {
//       stream[pendingAbortRequest_]!.resolve();
//       stream[pendingAbortRequest_] = undefined;
//     }
//   }
//   stream[shared.state_] = "closed";
//   const writer = stream[writer_];
//   if (writer !== undefined) {
//     writer[closedPromise_].resolve(undefined);
//   }
//   // Assert: stream.[[pendingAbortRequest]] is undefined.
//   // Assert: stream.[[storedError]] is undefined.
// }

// export function writableStreamFinishInFlightCloseWithError<InputType>(
//   stream: WritableStream<InputType>,
//   error: shared.ErrorResult
// ): void {
//   // Assert: stream.[[inFlightCloseRequest]] is not undefined.
//   stream[inFlightCloseRequest_]!.reject(error);
//   stream[inFlightCloseRequest_] = undefined;
//   // Assert: stream.[[state]] is "writable" or "erroring".
//   if (stream[pendingAbortRequest_] !== undefined) {
//     stream[pendingAbortRequest_]!.reject(error);
//     stream[pendingAbortRequest_] = undefined;
//   }
//   writableStreamDealWithRejection(stream, error);
// }

// export function writableStreamCloseQueuedOrInFlight<InputType>(
//   stream: WritableStream<InputType>
// ): boolean {
//   return (
//     stream[closeRequest_] !== undefined ||
//     stream[inFlightCloseRequest_] !== undefined
//   );
// }

// export function writableStreamHasOperationMarkedInFlight<InputType>(
//   stream: WritableStream<InputType>
// ): boolean {
//   return (
//     stream[inFlightWriteRequest_] !== undefined ||
//     stream[inFlightCloseRequest_] !== undefined
//   );
// }

// export function writableStreamMarkCloseRequestInFlight<InputType>(
//   stream: WritableStream<InputType>
// ): void {
//   // Assert: stream.[[inFlightCloseRequest]] is undefined.
//   // Assert: stream.[[closeRequest]] is not undefined.
//   stream[inFlightCloseRequest_] = stream[closeRequest_];
//   stream[closeRequest_] = undefined;
// }

// export function writableStreamMarkFirstWriteRequestInFlight<InputType>(
//   stream: WritableStream<InputType>
// ): void {
//   // Assert: stream.[[inFlightWriteRequest]] is undefined.
//   // Assert: stream.[[writeRequests]] is not empty.
//   const writeRequest = stream[writeRequests_].shift()!;
//   stream[inFlightWriteRequest_] = writeRequest;
// }

// export function writableStreamRejectCloseAndClosedPromiseIfNeeded<InputType>(
//   stream: WritableStream<InputType>
// ): void {
//   // Assert: stream.[[state]] is "errored".
//   const closeRequest = stream[closeRequest_];
//   if (closeRequest !== undefined) {
//     // Assert: stream.[[inFlightCloseRequest]] is undefined.
//     closeRequest.reject(stream[shared.storedError_]);
//     stream[closeRequest_] = undefined;
//   }
//   const writer = stream[writer_];
//   if (writer !== undefined) {
//     writer[closedPromise_].reject(stream[shared.storedError_]);
//     writer[closedPromise_].promise.catch(() => {});
//   }
// }

// export function writableStreamUpdateBackpressure<InputType>(
//   stream: WritableStream<InputType>,
//   backpressure: boolean
// ): void {
//   // Assert: stream.[[state]] is "writable".
//   // Assert: !WritableStreamCloseQueuedOrInFlight(stream) is false.
//   const writer = stream[writer_];
//   if (writer !== undefined && backpressure !== stream[backpressure_]) {
//     if (backpressure) {
//       writer[readyPromise_] = shared.createControlledPromise<void>();
//     } else {
//       writer[readyPromise_].resolve(undefined);
//     }
//   }
//   stream[backpressure_] = backpressure;
// }

// // ---- Writers

// export function isWritableStreamDefaultWriter(
//   value: unknown
// ): value is WritableStreamDefaultWriter<any> {
//   if (typeof value !== "object" || value === null) {
//     return false;
//   }
//   return ownerWritableStream_ in value;
// }

// export function writableStreamDefaultWriterAbort<InputType>(
//   writer: WritableStreamDefaultWriter<InputType>,
//   reason: shared.ErrorResult
// ): Promise<void> {
//   const stream = writer[ownerWritableStream_]!;
//   // Assert: stream is not undefined.
//   return writableStreamAbort(stream, reason);
// }

// export function writableStreamDefaultWriterClose<InputType>(
//   writer: WritableStreamDefaultWriter<InputType>
// ): Promise<void> {
//   const stream = writer[ownerWritableStream_]!;
//   // Assert: stream is not undefined.
//   const state = stream[shared.state_];
//   if (state === "closed" || state === "errored") {
//     return Promise.reject(
//       new TypeError("Writer stream is already closed or errored")
//     );
//   }
//   // Assert: state is "writable" or "erroring".
//   // Assert: writableStreamCloseQueuedOrInFlight(stream) is false.
//   const closePromise = shared.createControlledPromise<void>();
//   stream[closeRequest_] = closePromise;
//   if (stream[backpressure_] && state === "writable") {
//     writer[readyPromise_].resolve(undefined);
//   }
//   writableStreamDefaultControllerClose(stream[writableStreamController_]!);
//   return closePromise.promise;
// }

// export function writableStreamDefaultWriterCloseWithErrorPropagation<InputType>(
//   writer: WritableStreamDefaultWriter<InputType>
// ): Promise<void> {
//   const stream = writer[ownerWritableStream_]!;
//   // Assert: stream is not undefined.
//   const state = stream[shared.state_];
//   if (writableStreamCloseQueuedOrInFlight(stream) || state === "closed") {
//     return Promise.resolve(undefined);
//   }
//   if (state === "errored") {
//     return Promise.reject(stream[shared.storedError_]);
//   }
//   // Assert: state is "writable" or "erroring".
//   return writableStreamDefaultWriterClose(writer);
// }

// export function writableStreamDefaultWriterEnsureClosedPromiseRejected<
//   InputType
// >(
//   writer: WritableStreamDefaultWriter<InputType>,
//   error: shared.ErrorResult
// ): void {
//   const closedPromise = writer[closedPromise_];
//   if (closedPromise.state === shared.ControlledPromiseState.Pending) {
//     closedPromise.reject(error);
//   } else {
//     writer[closedPromise_] = shared.createControlledPromise<void>();
//     writer[closedPromise_].reject(error);
//   }
//   writer[closedPromise_].promise.catch(() => {});
// }

// export function writableStreamDefaultWriterEnsureReadyPromiseRejected<
//   InputType
// >(
//   writer: WritableStreamDefaultWriter<InputType>,
//   error: shared.ErrorResult
// ): void {
//   const readyPromise = writer[readyPromise_];
//   if (readyPromise.state === shared.ControlledPromiseState.Pending) {
//     readyPromise.reject(error);
//   } else {
//     writer[readyPromise_] = shared.createControlledPromise<void>();
//     writer[readyPromise_].reject(error);
//   }
//   writer[readyPromise_].promise.catch(() => {});
// }

// export function writableStreamDefaultWriterGetDesiredSize<InputType>(
//   writer: WritableStreamDefaultWriter<InputType>
// ): number | null {
//   const stream = writer[ownerWritableStream_]!;
//   const state = stream[shared.state_];
//   if (state === "errored" || state === "erroring") {
//     return null;
//   }
//   if (state === "closed") {
//     return 0;
//   }
//   return writableStreamDefaultControllerGetDesiredSize(
//     stream[writableStreamController_]!
//   );
// }

// export function writableStreamDefaultWriterRelease<InputType>(
//   writer: WritableStreamDefaultWriter<InputType>
// ): void {
//   const stream = writer[ownerWritableStream_]!;
//   // Assert: stream is not undefined.
//   // Assert: stream.[[writer]] is writer.
//   const releasedError = new TypeError();
//   writableStreamDefaultWriterEnsureReadyPromiseRejected(writer, releasedError);
//   writableStreamDefaultWriterEnsureClosedPromiseRejected(writer, releasedError);
//   stream[writer_] = undefined;
//   writer[ownerWritableStream_] = undefined;
// }

// export function writableStreamDefaultWriterWrite<InputType>(
//   writer: WritableStreamDefaultWriter<InputType>,
//   chunk: InputType
// ): Promise<void> {
//   const stream = writer[ownerWritableStream_]!;
//   // Assert: stream is not undefined.
//   const controller = stream[writableStreamController_]!;
//   const chunkSize = writableStreamDefaultControllerGetChunkSize(
//     controller,
//     chunk
//   );
//   if (writer[ownerWritableStream_] !== stream) {
//     return Promise.reject(new TypeError());
//   }
//   const state = stream[shared.state_];
//   if (state === "errored") {
//     return Promise.reject(stream[shared.storedError_]);
//   }
//   if (writableStreamCloseQueuedOrInFlight(stream) || state === "closed") {
//     return Promise.reject(
//       new TypeError("Cannot write to a closing or closed stream")
//     );
//   }
//   if (state === "erroring") {
//     return Promise.reject(stream[shared.storedError_]);
//   }
//   // Assert: state is "writable".
//   const promise = writableStreamAddWriteRequest(stream);
//   writableStreamDefaultControllerWrite(controller, chunk, chunkSize);
//   return promise;
// }

// // ---- Controller

// export function setUpWritableStreamDefaultController<InputType>(
//   stream: WritableStream<InputType>,
//   controller: WritableStreamDefaultController<InputType>,
//   startAlgorithm: StartAlgorithm,
//   writeAlgorithm: WriteAlgorithm<InputType>,
//   closeAlgorithm: CloseAlgorithm,
//   abortAlgorithm: AbortAlgorithm,
//   highWaterMark: number,
//   sizeAlgorithm: QueuingStrategySizeCallback<InputType>
// ): void {
//   if (!isWritableStream(stream)) {
//     throw new TypeError();
//   }
//   if (stream[writableStreamController_] !== undefined) {
//     throw new TypeError();
//   }

//   controller[controlledWritableStream_] = stream;
//   stream[writableStreamController_] = controller;
//   q.resetQueue(controller);
//   controller[started_] = false;
//   controller[strategySizeAlgorithm_] = sizeAlgorithm;
//   controller[strategyHWM_] = highWaterMark;
//   controller[writeAlgorithm_] = writeAlgorithm;
//   controller[closeAlgorithm_] = closeAlgorithm;
//   controller[abortAlgorithm_] = abortAlgorithm;
//   const backpressure = writableStreamDefaultControllerGetBackpressure(
//     controller
//   );
//   writableStreamUpdateBackpressure(stream, backpressure);

//   const startResult = startAlgorithm();
//   Promise.resolve(startResult).then(
//     _ => {
//       // Assert: stream.[[state]] is "writable" or "erroring".
//       controller[started_] = true;
//       writableStreamDefaultControllerAdvanceQueueIfNeeded(controller);
//     },
//     error => {
//       // Assert: stream.[[state]] is "writable" or "erroring".
//       controller[started_] = true;
//       writableStreamDealWithRejection(stream, error);
//     }
//   );
// }

// export function isWritableStreamDefaultController(
//   value: unknown
// ): value is WritableStreamDefaultController<any> {
//   if (typeof value !== "object" || value === null) {
//     return false;
//   }
//   return controlledWritableStream_ in value;
// }

// export function writableStreamDefaultControllerClearAlgorithms<InputType>(
//   controller: WritableStreamDefaultController<InputType>
// ): void {
//   // Use ! assertions to override type check here, this way we don't
//   // have to perform type checks/assertions everywhere else.
//   controller[writeAlgorithm_] = undefined!;
//   controller[closeAlgorithm_] = undefined!;
//   controller[abortAlgorithm_] = undefined!;
//   controller[strategySizeAlgorithm_] = undefined!;
// }

// export function writableStreamDefaultControllerClose<InputType>(
//   controller: WritableStreamDefaultController<InputType>
// ): void {
//   q.enqueueValueWithSize(controller, "close", 0);
//   writableStreamDefaultControllerAdvanceQueueIfNeeded(controller);
// }

// export function writableStreamDefaultControllerGetChunkSize<InputType>(
//   controller: WritableStreamDefaultController<InputType>,
//   chunk: InputType
// ): number {
//   let chunkSize: number;
//   try {
//     chunkSize = controller[strategySizeAlgorithm_](chunk);
//   } catch (error) {
//     writableStreamDefaultControllerErrorIfNeeded(controller, error);
//     chunkSize = 1;
//   }
//   return chunkSize;
// }

// export function writableStreamDefaultControllerGetDesiredSize<InputType>(
//   controller: WritableStreamDefaultController<InputType>
// ): number {
//   return controller[strategyHWM_] - controller[q.queueTotalSize_];
// }

// export function writableStreamDefaultControllerWrite<InputType>(
//   controller: WritableStreamDefaultController<InputType>,
//   chunk: InputType,
//   chunkSize: number
// ): void {
//   try {
//     q.enqueueValueWithSize(controller, { chunk }, chunkSize);
//   } catch (error) {
//     writableStreamDefaultControllerErrorIfNeeded(controller, error);
//     return;
//   }
//   const stream = controller[controlledWritableStream_];
//   if (
//     !writableStreamCloseQueuedOrInFlight(stream) &&
//     stream[shared.state_] === "writable"
//   ) {
//     const backpressure = writableStreamDefaultControllerGetBackpressure(
//       controller
//     );
//     writableStreamUpdateBackpressure(stream, backpressure);
//   }
//   writableStreamDefaultControllerAdvanceQueueIfNeeded(controller);
// }

// export function writableStreamDefaultControllerAdvanceQueueIfNeeded<InputType>(
//   controller: WritableStreamDefaultController<InputType>
// ): void {
//   if (!controller[started_]) {
//     return;
//   }
//   const stream = controller[controlledWritableStream_];
//   if (stream[inFlightWriteRequest_] !== undefined) {
//     return;
//   }
//   const state = stream[shared.state_];
//   if (state === "closed" || state === "errored") {
//     return;
//   }
//   if (state === "erroring") {
//     writableStreamFinishErroring(stream);
//     return;
//   }
//   if (controller[q.queue_].length === 0) {
//     return;
//   }
//   const writeRecord = q.peekQueueValue(controller);
//   if (writeRecord === "close") {
//     writableStreamDefaultControllerProcessClose(controller);
//   } else {
//     writableStreamDefaultControllerProcessWrite(controller, writeRecord.chunk);
//   }
// }

// export function writableStreamDefaultControllerErrorIfNeeded<InputType>(
//   controller: WritableStreamDefaultController<InputType>,
//   error: shared.ErrorResult
// ): void {
//   if (controller[controlledWritableStream_][shared.state_] === "writable") {
//     writableStreamDefaultControllerError(controller, error);
//   }
// }

// export function writableStreamDefaultControllerProcessClose<InputType>(
//   controller: WritableStreamDefaultController<InputType>
// ): void {
//   const stream = controller[controlledWritableStream_];
//   writableStreamMarkCloseRequestInFlight(stream);
//   q.dequeueValue(controller);
//   // Assert: controller.[[queue]] is empty.
//   const sinkClosePromise = controller[closeAlgorithm_]();
//   writableStreamDefaultControllerClearAlgorithms(controller);
//   sinkClosePromise.then(
//     _ => {
//       writableStreamFinishInFlightClose(stream);
//     },
//     error => {
//       writableStreamFinishInFlightCloseWithError(stream, error);
//     }
//   );
// }

// export function writableStreamDefaultControllerProcessWrite<InputType>(
//   controller: WritableStreamDefaultController<InputType>,
//   chunk: InputType
// ): void {
//   const stream = controller[controlledWritableStream_];
//   writableStreamMarkFirstWriteRequestInFlight(stream);
//   controller[writeAlgorithm_](chunk).then(
//     _ => {
//       writableStreamFinishInFlightWrite(stream);
//       const state = stream[shared.state_];
//       // 	Assert: state is "writable" or "erroring".
//       q.dequeueValue(controller);
//       if (
//         !writableStreamCloseQueuedOrInFlight(stream) &&
//         state === "writable"
//       ) {
//         const backpressure = writableStreamDefaultControllerGetBackpressure(
//           controller
//         );
//         writableStreamUpdateBackpressure(stream, backpressure);
//       }
//       writableStreamDefaultControllerAdvanceQueueIfNeeded(controller);
//     },
//     error => {
//       if (stream[shared.state_] === "writable") {
//         writableStreamDefaultControllerClearAlgorithms(controller);
//       }
//       writableStreamFinishInFlightWriteWithError(stream, error);
//     }
//   );
// }

// export function writableStreamDefaultControllerGetBackpressure<InputType>(
//   controller: WritableStreamDefaultController<InputType>
// ): boolean {
//   const desiredSize = writableStreamDefaultControllerGetDesiredSize(controller);
//   return desiredSize <= 0;
// }

// export function writableStreamDefaultControllerError<InputType>(
//   controller: WritableStreamDefaultController<InputType>,
//   error: shared.ErrorResult
// ): void {
//   const stream = controller[controlledWritableStream_];
//   // Assert: stream.[[state]] is "writable".
//   writableStreamDefaultControllerClearAlgorithms(controller);
//   writableStreamStartErroring(stream, error);
// }
