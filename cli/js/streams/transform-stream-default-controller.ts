// TODO reenable this code when we enable writableStreams and transport types
// // Forked from https://github.com/stardazed/sd-streams/tree/8928cf04b035fd02fb1340b7eb541c76be37e546
// // Copyright (c) 2018-Present by Arthur Langereis - @zenmumbler MIT

// /**
//  * streams/transform-stream-default-controller - TransformStreamDefaultController class implementation
//  * Part of Stardazed
//  * (c) 2018-Present by Arthur Langereis - @zenmumbler
//  * https://github.com/stardazed/sd-streams
//  */

// import * as rs from "./readable-internals.ts";
// import * as ts from "./transform-internals.ts";
// import { ErrorResult } from "./shared-internals.ts";

// export class TransformStreamDefaultController<InputType, OutputType>
//   implements ts.TransformStreamDefaultController<InputType, OutputType> {
//   [ts.controlledTransformStream_]: ts.TransformStream<InputType, OutputType>;
//   [ts.flushAlgorithm_]: ts.FlushAlgorithm;
//   [ts.transformAlgorithm_]: ts.TransformAlgorithm<InputType>;

//   constructor() {
//     throw new TypeError();
//   }

//   get desiredSize(): number | null {
//     if (!ts.isTransformStreamDefaultController(this)) {
//       throw new TypeError();
//     }
//     const readableController = this[ts.controlledTransformStream_][
//       ts.readable_
//     ][rs.readableStreamController_] as rs.SDReadableStreamDefaultController<
//       OutputType
//     >;
//     return rs.readableStreamDefaultControllerGetDesiredSize(readableController);
//   }

//   enqueue(chunk: OutputType): void {
//     if (!ts.isTransformStreamDefaultController(this)) {
//       throw new TypeError();
//     }
//     ts.transformStreamDefaultControllerEnqueue(this, chunk);
//   }

//   error(reason: ErrorResult): void {
//     if (!ts.isTransformStreamDefaultController(this)) {
//       throw new TypeError();
//     }
//     ts.transformStreamDefaultControllerError(this, reason);
//   }

//   terminate(): void {
//     if (!ts.isTransformStreamDefaultController(this)) {
//       throw new TypeError();
//     }
//     ts.transformStreamDefaultControllerTerminate(this);
//   }
// }
