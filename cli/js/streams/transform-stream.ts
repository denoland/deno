// TODO reenable this code when we enable writableStreams and transport types
// // Forked from https://github.com/stardazed/sd-streams/tree/8928cf04b035fd02fb1340b7eb541c76be37e546
// // Copyright (c) 2018-Present by Arthur Langereis - @zenmumbler MIT

// /**
//  * streams/transform-stream - TransformStream class implementation
//  * Part of Stardazed
//  * (c) 2018-Present by Arthur Langereis - @zenmumbler
//  * https://github.com/stardazed/sd-streams
//  */

// /* eslint-disable @typescript-eslint/no-explicit-any */
// // TODO reenable this lint here

// import * as rs from "./readable-internals.ts";
// import * as ws from "./writable-internals.ts";
// import * as ts from "./transform-internals.ts";
// import * as shared from "./shared-internals.ts";
// import { TransformStreamDefaultController } from "./transform-stream-default-controller.ts";
// import { QueuingStrategy } from "../dom_types.ts";

// export class TransformStream<InputType, OutputType> {
//   [ts.backpressure_]: boolean | undefined; // Whether there was backpressure on [[readable]] the last time it was observed
//   [ts.backpressureChangePromise_]: shared.ControlledPromise<void>; // A promise which is fulfilled and replaced every time the value of[[backpressure]] changes
//   [ts.readable_]: rs.SDReadableStream<OutputType>; // The ReadableStream instance controlled by this object
//   [ts.transformStreamController_]: TransformStreamDefaultController<
//     InputType,
//     OutputType
//   >; // A TransformStreamDefaultController created with the ability to control[[readable]] and[[writable]]; also used for the IsTransformStream brand check
//   [ts.writable_]: ws.WritableStream<InputType>; // The WritableStream instance controlled by this object

//   constructor(
//     transformer: ts.Transformer<InputType, OutputType> = {},
//     writableStrategy: QueuingStrategy<InputType> = {},
//     readableStrategy: QueuingStrategy<OutputType> = {}
//   ) {
//     const writableSizeFunction = writableStrategy.size;
//     const writableHighWaterMark = writableStrategy.highWaterMark;
//     const readableSizeFunction = readableStrategy.size;
//     const readableHighWaterMark = readableStrategy.highWaterMark;

//     const writableType = transformer.writableType;
//     if (writableType !== undefined) {
//       throw new RangeError(
//         "The transformer's `writableType` field must be undefined"
//       );
//     }
//     const writableSizeAlgorithm = shared.makeSizeAlgorithmFromSizeFunction(
//       writableSizeFunction
//     );
//     const writableHWM = shared.validateAndNormalizeHighWaterMark(
//       writableHighWaterMark === undefined ? 1 : writableHighWaterMark
//     );

//     const readableType = transformer.readableType;
//     if (readableType !== undefined) {
//       throw new RangeError(
//         "The transformer's `readableType` field must be undefined"
//       );
//     }
//     const readableSizeAlgorithm = shared.makeSizeAlgorithmFromSizeFunction(
//       readableSizeFunction
//     );
//     const readableHWM = shared.validateAndNormalizeHighWaterMark(
//       readableHighWaterMark === undefined ? 0 : readableHighWaterMark
//     );

//     const startPromise = shared.createControlledPromise<void>();
//     ts.initializeTransformStream(
//       this,
//       startPromise.promise,
//       writableHWM,
//       writableSizeAlgorithm,
//       readableHWM,
//       readableSizeAlgorithm
//     );
//     setUpTransformStreamDefaultControllerFromTransformer(this, transformer);

//     const startResult = shared.invokeOrNoop(transformer, "start", [
//       this[ts.transformStreamController_]
//     ]);
//     startPromise.resolve(startResult);
//   }

//   get readable(): rs.SDReadableStream<OutputType> {
//     if (!ts.isTransformStream(this)) {
//       throw new TypeError();
//     }
//     return this[ts.readable_];
//   }

//   get writable(): ws.WritableStream<InputType> {
//     if (!ts.isTransformStream(this)) {
//       throw new TypeError();
//     }
//     return this[ts.writable_];
//   }
// }

// function setUpTransformStreamDefaultControllerFromTransformer<
//   InputType,
//   OutputType
// >(
//   stream: TransformStream<InputType, OutputType>,
//   transformer: ts.Transformer<InputType, OutputType>
// ): void {
//   const controller = Object.create(
//     TransformStreamDefaultController.prototype
//   ) as TransformStreamDefaultController<InputType, OutputType>;
//   let transformAlgorithm: ts.TransformAlgorithm<InputType>;

//   const transformMethod = transformer.transform;
//   if (transformMethod !== undefined) {
//     if (typeof transformMethod !== "function") {
//       throw new TypeError(
//         "`transform` field of the transformer must be a function"
//       );
//     }
//     transformAlgorithm = (chunk: InputType): Promise<any> =>
//       shared.promiseCall(transformMethod, transformer, [chunk, controller]);
//   } else {
//     // use identity transform
//     transformAlgorithm = function(chunk: InputType): Promise<void> {
//       try {
//         // OutputType and InputType are the same here
//         ts.transformStreamDefaultControllerEnqueue(
//           controller,
//           (chunk as unknown) as OutputType
//         );
//       } catch (error) {
//         return Promise.reject(error);
//       }
//       return Promise.resolve(undefined);
//     };
//   }
//   const flushAlgorithm = shared.createAlgorithmFromUnderlyingMethod(
//     transformer,
//     "flush",
//     [controller]
//   );
//   ts.setUpTransformStreamDefaultController(
//     stream,
//     controller,
//     transformAlgorithm,
//     flushAlgorithm
//   );
// }
