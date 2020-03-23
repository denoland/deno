// TODO reenable this code when we enable writableStreams and transport types
// // Forked from https://github.com/stardazed/sd-streams/tree/8928cf04b035fd02fb1340b7eb541c76be37e546
// // Copyright (c) 2018-Present by Arthur Langereis - @zenmumbler MIT

// /**
//  * streams/pipe-to - pipeTo algorithm implementation
//  * Part of Stardazed
//  * (c) 2018-Present by Arthur Langereis - @zenmumbler
//  * https://github.com/stardazed/sd-streams
//  */

// /* eslint-disable @typescript-eslint/no-explicit-any */
// // TODO reenable this lint here

// import * as rs from "./readable-internals.ts";
// import * as ws from "./writable-internals.ts";
// import * as shared from "./shared-internals.ts";

// import { ReadableStreamDefaultReader } from "./readable-stream-default-reader.ts";
// import { WritableStreamDefaultWriter } from "./writable-stream-default-writer.ts";
// import { PipeOptions } from "../dom_types.ts";
// import { Err } from "../errors.ts";

// // add a wrapper to handle falsy rejections
// interface ErrorWrapper {
//   actualError: shared.ErrorResult;
// }

// export function pipeTo<ChunkType>(
//   source: rs.SDReadableStream<ChunkType>,
//   dest: ws.WritableStream<ChunkType>,
//   options: PipeOptions
// ): Promise<void> {
//   const preventClose = !!options.preventClose;
//   const preventAbort = !!options.preventAbort;
//   const preventCancel = !!options.preventCancel;
//   const signal = options.signal;

//   let shuttingDown = false;
//   let latestWrite = Promise.resolve();
//   const promise = shared.createControlledPromise<void>();

//   // If IsReadableByteStreamController(this.[[readableStreamController]]) is true, let reader be either ! AcquireReadableStreamBYOBReader(this) or ! AcquireReadableStreamDefaultReader(this), at the user agent’s discretion.
//   // Otherwise, let reader be ! AcquireReadableStreamDefaultReader(this).
//   const reader = new ReadableStreamDefaultReader(source);
//   const writer = new WritableStreamDefaultWriter(dest);

//   let abortAlgorithm: () => any;
//   if (signal !== undefined) {
//     abortAlgorithm = (): void => {
//       // TODO this should be a DOMException,
//       // https://github.com/stardazed/sd-streams/blob/master/packages/streams/src/pipe-to.ts#L38
//       const error = new errors.Aborted("Aborted");
//       const actions: Array<() => Promise<void>> = [];
//       if (preventAbort === false) {
//         actions.push(() => {
//           if (dest[shared.state_] === "writable") {
//             return ws.writableStreamAbort(dest, error);
//           }
//           return Promise.resolve();
//         });
//       }
//       if (preventCancel === false) {
//         actions.push(() => {
//           if (source[shared.state_] === "readable") {
//             return rs.readableStreamCancel(source, error);
//           }
//           return Promise.resolve();
//         });
//       }
//       shutDown(
//         () => {
//           return Promise.all(actions.map(a => a())).then(_ => undefined);
//         },
//         { actualError: error }
//       );
//     };

//     if (signal.aborted === true) {
//       abortAlgorithm();
//     } else {
//       signal.addEventListener("abort", abortAlgorithm);
//     }
//   }

//   function onStreamErrored(
//     stream: rs.SDReadableStream<ChunkType> | ws.WritableStream<ChunkType>,
//     promise: Promise<void>,
//     action: (error: shared.ErrorResult) => void
//   ): void {
//     if (stream[shared.state_] === "errored") {
//       action(stream[shared.storedError_]);
//     } else {
//       promise.catch(action);
//     }
//   }

//   function onStreamClosed(
//     stream: rs.SDReadableStream<ChunkType> | ws.WritableStream<ChunkType>,
//     promise: Promise<void>,
//     action: () => void
//   ): void {
//     if (stream[shared.state_] === "closed") {
//       action();
//     } else {
//       promise.then(action);
//     }
//   }

//   onStreamErrored(source, reader[rs.closedPromise_].promise, error => {
//     if (!preventAbort) {
//       shutDown(() => ws.writableStreamAbort(dest, error), {
//         actualError: error
//       });
//     } else {
//       shutDown(undefined, { actualError: error });
//     }
//   });

//   onStreamErrored(dest, writer[ws.closedPromise_].promise, error => {
//     if (!preventCancel) {
//       shutDown(() => rs.readableStreamCancel(source, error), {
//         actualError: error
//       });
//     } else {
//       shutDown(undefined, { actualError: error });
//     }
//   });

//   onStreamClosed(source, reader[rs.closedPromise_].promise, () => {
//     if (!preventClose) {
//       shutDown(() =>
//         ws.writableStreamDefaultWriterCloseWithErrorPropagation(writer)
//       );
//     } else {
//       shutDown();
//     }
//   });

//   if (
//     ws.writableStreamCloseQueuedOrInFlight(dest) ||
//     dest[shared.state_] === "closed"
//   ) {
//     // Assert: no chunks have been read or written.
//     const destClosed = new TypeError();
//     if (!preventCancel) {
//       shutDown(() => rs.readableStreamCancel(source, destClosed), {
//         actualError: destClosed
//       });
//     } else {
//       shutDown(undefined, { actualError: destClosed });
//     }
//   }

//   function awaitLatestWrite(): Promise<void> {
//     const curLatestWrite = latestWrite;
//     return latestWrite.then(() =>
//       curLatestWrite === latestWrite ? undefined : awaitLatestWrite()
//     );
//   }

//   function flushRemainder(): Promise<void> | undefined {
//     if (
//       dest[shared.state_] === "writable" &&
//       !ws.writableStreamCloseQueuedOrInFlight(dest)
//     ) {
//       return awaitLatestWrite();
//     } else {
//       return undefined;
//     }
//   }

//   function shutDown(action?: () => Promise<void>, error?: ErrorWrapper): void {
//     if (shuttingDown) {
//       return;
//     }
//     shuttingDown = true;

//     if (action === undefined) {
//       action = (): Promise<void> => Promise.resolve();
//     }

//     function finishShutDown(): void {
//       action!().then(
//         _ => finalize(error),
//         newError => finalize({ actualError: newError })
//       );
//     }

//     const flushWait = flushRemainder();
//     if (flushWait) {
//       flushWait.then(finishShutDown);
//     } else {
//       finishShutDown();
//     }
//   }

//   function finalize(error?: ErrorWrapper): void {
//     ws.writableStreamDefaultWriterRelease(writer);
//     rs.readableStreamReaderGenericRelease(reader);
//     if (signal && abortAlgorithm) {
//       signal.removeEventListener("abort", abortAlgorithm);
//     }
//     if (error) {
//       promise.reject(error.actualError);
//     } else {
//       promise.resolve(undefined);
//     }
//   }

//   function next(): Promise<void> | undefined {
//     if (shuttingDown) {
//       return;
//     }

//     writer[ws.readyPromise_].promise.then(() => {
//       rs.readableStreamDefaultReaderRead(reader).then(
//         ({ value, done }) => {
//           if (done) {
//             return;
//           }
//           latestWrite = ws
//             .writableStreamDefaultWriterWrite(writer, value!)
//             .catch(() => {});
//           next();
//         },
//         _error => {
//           latestWrite = Promise.resolve();
//         }
//       );
//     });
//   }

//   next();

//   return promise.promise;
// }
