// TODO reenable this code when we enable writableStreams and transport types
// // Forked from https://github.com/stardazed/sd-streams/tree/8928cf04b035fd02fb1340b7eb541c76be37e546
// // Copyright (c) 2018-Present by Arthur Langereis - @zenmumbler MIT

// /**
//  * streams/transform-internals - internal types and functions for transform streams
//  * Part of Stardazed
//  * (c) 2018-Present by Arthur Langereis - @zenmumbler
//  * https://github.com/stardazed/sd-streams
//  */

// /* eslint-disable @typescript-eslint/no-explicit-any */
// // TODO reenable this lint here

// import * as rs from "./readable-internals.ts";
// import * as ws from "./writable-internals.ts";
// import * as shared from "./shared-internals.ts";

// import { createReadableStream } from "./readable-stream.ts";
// import { createWritableStream } from "./writable-stream.ts";

// import { QueuingStrategy, QueuingStrategySizeCallback } from "../dom_types.ts";

// export const state_ = Symbol("transformState_");
// export const backpressure_ = Symbol("backpressure_");
// export const backpressureChangePromise_ = Symbol("backpressureChangePromise_");
// export const readable_ = Symbol("readable_");
// export const transformStreamController_ = Symbol("transformStreamController_");
// export const writable_ = Symbol("writable_");

// export const controlledTransformStream_ = Symbol("controlledTransformStream_");
// export const flushAlgorithm_ = Symbol("flushAlgorithm_");
// export const transformAlgorithm_ = Symbol("transformAlgorithm_");

// // ----

// export type TransformFunction<InputType, OutputType> = (
//   chunk: InputType,
//   controller: TransformStreamDefaultController<InputType, OutputType>
// ) => void | PromiseLike<void>;
// export type TransformAlgorithm<InputType> = (chunk: InputType) => Promise<void>;
// export type FlushFunction<InputType, OutputType> = (
//   controller: TransformStreamDefaultController<InputType, OutputType>
// ) => void | PromiseLike<void>;
// export type FlushAlgorithm = () => Promise<void>;

// // ----

// export interface TransformStreamDefaultController<InputType, OutputType> {
//   readonly desiredSize: number | null;
//   enqueue(chunk: OutputType): void;
//   error(reason: shared.ErrorResult): void;
//   terminate(): void;

//   [controlledTransformStream_]: TransformStream<InputType, OutputType>; // The TransformStream instance controlled; also used for the IsTransformStreamDefaultController brand check
//   [flushAlgorithm_]: FlushAlgorithm; // A promise - returning algorithm which communicates a requested close to the transformer
//   [transformAlgorithm_]: TransformAlgorithm<InputType>; // A promise - returning algorithm, taking one argument(the chunk to transform), which requests the transformer perform its transformation
// }

// export interface Transformer<InputType, OutputType> {
//   start?(
//     controller: TransformStreamDefaultController<InputType, OutputType>
//   ): void | PromiseLike<void>;
//   transform?: TransformFunction<InputType, OutputType>;
//   flush?: FlushFunction<InputType, OutputType>;

//   readableType?: undefined; // for future spec changes
//   writableType?: undefined; // for future spec changes
// }

// export declare class TransformStream<InputType, OutputType> {
//   constructor(
//     transformer: Transformer<InputType, OutputType>,
//     writableStrategy: QueuingStrategy<InputType>,
//     readableStrategy: QueuingStrategy<OutputType>
//   );

//   readonly readable: rs.SDReadableStream<OutputType>;
//   readonly writable: ws.WritableStream<InputType>;

//   [backpressure_]: boolean | undefined; // Whether there was backpressure on [[readable]] the last time it was observed
//   [backpressureChangePromise_]: shared.ControlledPromise<void> | undefined; // A promise which is fulfilled and replaced every time the value of[[backpressure]] changes
//   [readable_]: rs.SDReadableStream<OutputType>; // The ReadableStream instance controlled by this object
//   [transformStreamController_]: TransformStreamDefaultController<
//     InputType,
//     OutputType
//   >; // A TransformStreamDefaultController created with the ability to control[[readable]] and[[writable]]; also used for the IsTransformStream brand check
//   [writable_]: ws.WritableStream<InputType>; // The WritableStream instance controlled by this object
// }

// // ---- TransformStream

// export function isTransformStream(
//   value: unknown
// ): value is TransformStream<any, any> {
//   if (typeof value !== "object" || value === null) {
//     return false;
//   }
//   return transformStreamController_ in value;
// }

// export function initializeTransformStream<InputType, OutputType>(
//   stream: TransformStream<InputType, OutputType>,
//   startPromise: Promise<void>,
//   writableHighWaterMark: number,
//   writableSizeAlgorithm: QueuingStrategySizeCallback<InputType>,
//   readableHighWaterMark: number,
//   readableSizeAlgorithm: QueuingStrategySizeCallback<OutputType>
// ): void {
//   const startAlgorithm = function(): Promise<void> {
//     return startPromise;
//   };
//   const writeAlgorithm = function(chunk: InputType): Promise<void> {
//     return transformStreamDefaultSinkWriteAlgorithm(stream, chunk);
//   };
//   const abortAlgorithm = function(reason: shared.ErrorResult): Promise<void> {
//     return transformStreamDefaultSinkAbortAlgorithm(stream, reason);
//   };
//   const closeAlgorithm = function(): Promise<void> {
//     return transformStreamDefaultSinkCloseAlgorithm(stream);
//   };
//   stream[writable_] = createWritableStream<InputType>(
//     startAlgorithm,
//     writeAlgorithm,
//     closeAlgorithm,
//     abortAlgorithm,
//     writableHighWaterMark,
//     writableSizeAlgorithm
//   );

//   const pullAlgorithm = function(): Promise<void> {
//     return transformStreamDefaultSourcePullAlgorithm(stream);
//   };
//   const cancelAlgorithm = function(
//     reason: shared.ErrorResult
//   ): Promise<undefined> {
//     transformStreamErrorWritableAndUnblockWrite(stream, reason);
//     return Promise.resolve(undefined);
//   };
//   stream[readable_] = createReadableStream(
//     startAlgorithm,
//     pullAlgorithm,
//     cancelAlgorithm,
//     readableHighWaterMark,
//     readableSizeAlgorithm
//   );

//   stream[backpressure_] = undefined;
//   stream[backpressureChangePromise_] = undefined;
//   transformStreamSetBackpressure(stream, true);
//   stream[transformStreamController_] = undefined!; // initialize slot for brand-check
// }

// export function transformStreamError<InputType, OutputType>(
//   stream: TransformStream<InputType, OutputType>,
//   error: shared.ErrorResult
// ): void {
//   rs.readableStreamDefaultControllerError(
//     stream[readable_][
//       rs.readableStreamController_
//     ] as rs.SDReadableStreamDefaultController<OutputType>,
//     error
//   );
//   transformStreamErrorWritableAndUnblockWrite(stream, error);
// }

// export function transformStreamErrorWritableAndUnblockWrite<
//   InputType,
//   OutputType
// >(
//   stream: TransformStream<InputType, OutputType>,
//   error: shared.ErrorResult
// ): void {
//   transformStreamDefaultControllerClearAlgorithms(
//     stream[transformStreamController_]
//   );
//   ws.writableStreamDefaultControllerErrorIfNeeded(
//     stream[writable_][ws.writableStreamController_]!,
//     error
//   );
//   if (stream[backpressure_]) {
//     transformStreamSetBackpressure(stream, false);
//   }
// }

// export function transformStreamSetBackpressure<InputType, OutputType>(
//   stream: TransformStream<InputType, OutputType>,
//   backpressure: boolean
// ): void {
//   // Assert: stream.[[backpressure]] is not backpressure.
//   if (stream[backpressure_] !== undefined) {
//     stream[backpressureChangePromise_]!.resolve(undefined);
//   }
//   stream[backpressureChangePromise_] = shared.createControlledPromise<void>();
//   stream[backpressure_] = backpressure;
// }

// // ---- TransformStreamDefaultController

// export function isTransformStreamDefaultController(
//   value: unknown
// ): value is TransformStreamDefaultController<any, any> {
//   if (typeof value !== "object" || value === null) {
//     return false;
//   }
//   return controlledTransformStream_ in value;
// }

// export function setUpTransformStreamDefaultController<InputType, OutputType>(
//   stream: TransformStream<InputType, OutputType>,
//   controller: TransformStreamDefaultController<InputType, OutputType>,
//   transformAlgorithm: TransformAlgorithm<InputType>,
//   flushAlgorithm: FlushAlgorithm
// ): void {
//   // Assert: ! IsTransformStream(stream) is true.
//   // Assert: stream.[[transformStreamController]] is undefined.
//   controller[controlledTransformStream_] = stream;
//   stream[transformStreamController_] = controller;
//   controller[transformAlgorithm_] = transformAlgorithm;
//   controller[flushAlgorithm_] = flushAlgorithm;
// }

// export function transformStreamDefaultControllerClearAlgorithms<
//   InputType,
//   OutputType
// >(controller: TransformStreamDefaultController<InputType, OutputType>): void {
//   // Use ! assertions to override type check here, this way we don't
//   // have to perform type checks/assertions everywhere else.
//   controller[transformAlgorithm_] = undefined!;
//   controller[flushAlgorithm_] = undefined!;
// }

// export function transformStreamDefaultControllerEnqueue<InputType, OutputType>(
//   controller: TransformStreamDefaultController<InputType, OutputType>,
//   chunk: OutputType
// ): void {
//   const stream = controller[controlledTransformStream_];
//   const readableController = stream[readable_][
//     rs.readableStreamController_
//   ] as rs.SDReadableStreamDefaultController<OutputType>;
//   if (
//     !rs.readableStreamDefaultControllerCanCloseOrEnqueue(readableController)
//   ) {
//     throw new TypeError();
//   }
//   try {
//     rs.readableStreamDefaultControllerEnqueue(readableController, chunk);
//   } catch (error) {
//     transformStreamErrorWritableAndUnblockWrite(stream, error);
//     throw stream[readable_][shared.storedError_];
//   }
//   const backpressure = rs.readableStreamDefaultControllerHasBackpressure(
//     readableController
//   );
//   if (backpressure !== stream[backpressure_]) {
//     // Assert: backpressure is true.
//     transformStreamSetBackpressure(stream, true);
//   }
// }

// export function transformStreamDefaultControllerError<InputType, OutputType>(
//   controller: TransformStreamDefaultController<InputType, OutputType>,
//   error: shared.ErrorResult
// ): void {
//   transformStreamError(controller[controlledTransformStream_], error);
// }

// export function transformStreamDefaultControllerPerformTransform<
//   InputType,
//   OutputType
// >(
//   controller: TransformStreamDefaultController<InputType, OutputType>,
//   chunk: InputType
// ): Promise<void> {
//   const transformPromise = controller[transformAlgorithm_](chunk);
//   return transformPromise.catch(error => {
//     transformStreamError(controller[controlledTransformStream_], error);
//     throw error;
//   });
// }

// export function transformStreamDefaultControllerTerminate<
//   InputType,
//   OutputType
// >(controller: TransformStreamDefaultController<InputType, OutputType>): void {
//   const stream = controller[controlledTransformStream_];
//   const readableController = stream[readable_][
//     rs.readableStreamController_
//   ] as rs.SDReadableStreamDefaultController<OutputType>;
//   if (rs.readableStreamDefaultControllerCanCloseOrEnqueue(readableController)) {
//     rs.readableStreamDefaultControllerClose(readableController);
//   }
//   const error = new TypeError("The transform stream has been terminated");
//   transformStreamErrorWritableAndUnblockWrite(stream, error);
// }

// // ---- Transform Sinks

// export function transformStreamDefaultSinkWriteAlgorithm<InputType, OutputType>(
//   stream: TransformStream<InputType, OutputType>,
//   chunk: InputType
// ): Promise<void> {
//   // Assert: stream.[[writable]].[[state]] is "writable".
//   const controller = stream[transformStreamController_];
//   if (stream[backpressure_]) {
//     const backpressureChangePromise = stream[backpressureChangePromise_]!;
//     // Assert: backpressureChangePromise is not undefined.
//     return backpressureChangePromise.promise.then(_ => {
//       const writable = stream[writable_];
//       const state = writable[shared.state_];
//       if (state === "erroring") {
//         throw writable[shared.storedError_];
//       }
//       // Assert: state is "writable".
//       return transformStreamDefaultControllerPerformTransform(
//         controller,
//         chunk
//       );
//     });
//   }
//   return transformStreamDefaultControllerPerformTransform(controller, chunk);
// }

// export function transformStreamDefaultSinkAbortAlgorithm<InputType, OutputType>(
//   stream: TransformStream<InputType, OutputType>,
//   reason: shared.ErrorResult
// ): Promise<void> {
//   transformStreamError(stream, reason);
//   return Promise.resolve(undefined);
// }

// export function transformStreamDefaultSinkCloseAlgorithm<InputType, OutputType>(
//   stream: TransformStream<InputType, OutputType>
// ): Promise<void> {
//   const readable = stream[readable_];
//   const controller = stream[transformStreamController_];
//   const flushPromise = controller[flushAlgorithm_]();
//   transformStreamDefaultControllerClearAlgorithms(controller);

//   return flushPromise.then(
//     _ => {
//       if (readable[shared.state_] === "errored") {
//         throw readable[shared.storedError_];
//       }
//       const readableController = readable[
//         rs.readableStreamController_
//       ] as rs.SDReadableStreamDefaultController<OutputType>;
//       if (
//         rs.readableStreamDefaultControllerCanCloseOrEnqueue(readableController)
//       ) {
//         rs.readableStreamDefaultControllerClose(readableController);
//       }
//     },
//     error => {
//       transformStreamError(stream, error);
//       throw readable[shared.storedError_];
//     }
//   );
// }

// // ---- Transform Sources

// export function transformStreamDefaultSourcePullAlgorithm<
//   InputType,
//   OutputType
// >(stream: TransformStream<InputType, OutputType>): Promise<void> {
//   // Assert: stream.[[backpressure]] is true.
//   // Assert: stream.[[backpressureChangePromise]] is not undefined.
//   transformStreamSetBackpressure(stream, false);
//   return stream[backpressureChangePromise_]!.promise;
// }
