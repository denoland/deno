// TODO reenable this code when we enable writableStreams and transport types
// // Forked from https://github.com/stardazed/sd-streams/tree/8928cf04b035fd02fb1340b7eb541c76be37e546
// // Copyright (c) 2018-Present by Arthur Langereis - @zenmumbler MIT

// /**
//  * streams/writable-stream - WritableStream class implementation
//  * Part of Stardazed
//  * (c) 2018-Present by Arthur Langereis - @zenmumbler
//  * https://github.com/stardazed/sd-streams
//  */

// import * as ws from "./writable-internals.ts";
// import * as shared from "./shared-internals.ts";
// import {
//   WritableStreamDefaultController,
//   setUpWritableStreamDefaultControllerFromUnderlyingSink
// } from "./writable-stream-default-controller.ts";
// import { WritableStreamDefaultWriter } from "./writable-stream-default-writer.ts";
// import { QueuingStrategy, QueuingStrategySizeCallback } from "../dom_types.ts";

// export class WritableStream<InputType> {
//   [shared.state_]: ws.WritableStreamState;
//   [shared.storedError_]: shared.ErrorResult;
//   [ws.backpressure_]: boolean;
//   [ws.closeRequest_]: shared.ControlledPromise<void> | undefined;
//   [ws.inFlightWriteRequest_]: shared.ControlledPromise<void> | undefined;
//   [ws.inFlightCloseRequest_]: shared.ControlledPromise<void> | undefined;
//   [ws.pendingAbortRequest_]: ws.AbortRequest | undefined;
//   [ws.writableStreamController_]:
//     | ws.WritableStreamDefaultController<InputType>
//     | undefined;
//   [ws.writer_]: ws.WritableStreamDefaultWriter<InputType> | undefined;
//   [ws.writeRequests_]: Array<shared.ControlledPromise<void>>;

//   constructor(
//     sink: ws.WritableStreamSink<InputType> = {},
//     strategy: QueuingStrategy<InputType> = {}
//   ) {
//     ws.initializeWritableStream(this);
//     const sizeFunc = strategy.size;
//     const stratHWM = strategy.highWaterMark;
//     if (sink.type !== undefined) {
//       throw new RangeError("The type of an underlying sink must be undefined");
//     }

//     const sizeAlgorithm = shared.makeSizeAlgorithmFromSizeFunction(sizeFunc);
//     const highWaterMark = shared.validateAndNormalizeHighWaterMark(
//       stratHWM === undefined ? 1 : stratHWM
//     );

//     setUpWritableStreamDefaultControllerFromUnderlyingSink(
//       this,
//       sink,
//       highWaterMark,
//       sizeAlgorithm
//     );
//   }

//   get locked(): boolean {
//     if (!ws.isWritableStream(this)) {
//       throw new TypeError();
//     }
//     return ws.isWritableStreamLocked(this);
//   }

//   abort(reason?: shared.ErrorResult): Promise<void> {
//     if (!ws.isWritableStream(this)) {
//       return Promise.reject(new TypeError());
//     }
//     if (ws.isWritableStreamLocked(this)) {
//       return Promise.reject(new TypeError("Cannot abort a locked stream"));
//     }
//     return ws.writableStreamAbort(this, reason);
//   }

//   getWriter(): ws.WritableStreamWriter<InputType> {
//     if (!ws.isWritableStream(this)) {
//       throw new TypeError();
//     }
//     return new WritableStreamDefaultWriter(this);
//   }
// }

// export function createWritableStream<InputType>(
//   startAlgorithm: ws.StartAlgorithm,
//   writeAlgorithm: ws.WriteAlgorithm<InputType>,
//   closeAlgorithm: ws.CloseAlgorithm,
//   abortAlgorithm: ws.AbortAlgorithm,
//   highWaterMark?: number,
//   sizeAlgorithm?: QueuingStrategySizeCallback<InputType>
// ): WritableStream<InputType> {
//   if (highWaterMark === undefined) {
//     highWaterMark = 1;
//   }
//   if (sizeAlgorithm === undefined) {
//     sizeAlgorithm = (): number => 1;
//   }
//   // Assert: ! IsNonNegativeNumber(highWaterMark) is true.

//   const stream = Object.create(WritableStream.prototype) as WritableStream<
//     InputType
//   >;
//   ws.initializeWritableStream(stream);
//   const controller = Object.create(
//     WritableStreamDefaultController.prototype
//   ) as WritableStreamDefaultController<InputType>;
//   ws.setUpWritableStreamDefaultController(
//     stream,
//     controller,
//     startAlgorithm,
//     writeAlgorithm,
//     closeAlgorithm,
//     abortAlgorithm,
//     highWaterMark,
//     sizeAlgorithm
//   );
//   return stream;
// }
