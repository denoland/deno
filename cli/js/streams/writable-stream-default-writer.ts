// TODO reenable this code when we enable writableStreams and transport types
// // Forked from https://github.com/stardazed/sd-streams/tree/8928cf04b035fd02fb1340b7eb541c76be37e546
// // Copyright (c) 2018-Present by Arthur Langereis - @zenmumbler MIT

// /**
//  * streams/writable-stream-default-writer - WritableStreamDefaultWriter class implementation
//  * Part of Stardazed
//  * (c) 2018-Present by Arthur Langereis - @zenmumbler
//  * https://github.com/stardazed/sd-streams
//  */

// import * as ws from "./writable-internals.ts";
// import * as shared from "./shared-internals.ts";

// export class WritableStreamDefaultWriter<InputType>
//   implements ws.WritableStreamDefaultWriter<InputType> {
//   [ws.ownerWritableStream_]: ws.WritableStream<InputType> | undefined;
//   [ws.readyPromise_]: shared.ControlledPromise<void>;
//   [ws.closedPromise_]: shared.ControlledPromise<void>;

//   constructor(stream: ws.WritableStream<InputType>) {
//     if (!ws.isWritableStream(stream)) {
//       throw new TypeError();
//     }
//     if (ws.isWritableStreamLocked(stream)) {
//       throw new TypeError("Stream is already locked");
//     }
//     this[ws.ownerWritableStream_] = stream;
//     stream[ws.writer_] = this;

//     const readyPromise = shared.createControlledPromise<void>();
//     const closedPromise = shared.createControlledPromise<void>();
//     this[ws.readyPromise_] = readyPromise;
//     this[ws.closedPromise_] = closedPromise;

//     const state = stream[shared.state_];
//     if (state === "writable") {
//       if (
//         !ws.writableStreamCloseQueuedOrInFlight(stream) &&
//         stream[ws.backpressure_]
//       ) {
//         // OK Set this.[[readyPromise]] to a new promise.
//       } else {
//         readyPromise.resolve(undefined);
//       }
//       // OK Set this.[[closedPromise]] to a new promise.
//     } else if (state === "erroring") {
//       readyPromise.reject(stream[shared.storedError_]);
//       readyPromise.promise.catch(() => {});
//       // OK Set this.[[closedPromise]] to a new promise.
//     } else if (state === "closed") {
//       readyPromise.resolve(undefined);
//       closedPromise.resolve(undefined);
//     } else {
//       // Assert: state is "errored".
//       const storedError = stream[shared.storedError_];
//       readyPromise.reject(storedError);
//       readyPromise.promise.catch(() => {});
//       closedPromise.reject(storedError);
//       closedPromise.promise.catch(() => {});
//     }
//   }

//   abort(reason: shared.ErrorResult): Promise<void> {
//     if (!ws.isWritableStreamDefaultWriter(this)) {
//       return Promise.reject(new TypeError());
//     }
//     if (this[ws.ownerWritableStream_] === undefined) {
//       return Promise.reject(
//         new TypeError("Writer is not connected to a stream")
//       );
//     }
//     return ws.writableStreamDefaultWriterAbort(this, reason);
//   }

//   close(): Promise<void> {
//     if (!ws.isWritableStreamDefaultWriter(this)) {
//       return Promise.reject(new TypeError());
//     }
//     const stream = this[ws.ownerWritableStream_];
//     if (stream === undefined) {
//       return Promise.reject(
//         new TypeError("Writer is not connected to a stream")
//       );
//     }
//     if (ws.writableStreamCloseQueuedOrInFlight(stream)) {
//       return Promise.reject(new TypeError());
//     }
//     return ws.writableStreamDefaultWriterClose(this);
//   }

//   releaseLock(): void {
//     const stream = this[ws.ownerWritableStream_];
//     if (stream === undefined) {
//       return;
//     }
//     // Assert: stream.[[writer]] is not undefined.
//     ws.writableStreamDefaultWriterRelease(this);
//   }

//   write(chunk: InputType): Promise<void> {
//     if (!ws.isWritableStreamDefaultWriter(this)) {
//       return Promise.reject(new TypeError());
//     }
//     if (this[ws.ownerWritableStream_] === undefined) {
//       return Promise.reject(
//         new TypeError("Writer is not connected to a stream")
//       );
//     }
//     return ws.writableStreamDefaultWriterWrite(this, chunk);
//   }

//   get closed(): Promise<void> {
//     if (!ws.isWritableStreamDefaultWriter(this)) {
//       return Promise.reject(new TypeError());
//     }
//     return this[ws.closedPromise_].promise;
//   }

//   get desiredSize(): number | null {
//     if (!ws.isWritableStreamDefaultWriter(this)) {
//       throw new TypeError();
//     }
//     if (this[ws.ownerWritableStream_] === undefined) {
//       throw new TypeError("Writer is not connected to stream");
//     }
//     return ws.writableStreamDefaultWriterGetDesiredSize(this);
//   }

//   get ready(): Promise<void> {
//     if (!ws.isWritableStreamDefaultWriter(this)) {
//       return Promise.reject(new TypeError());
//     }
//     return this[ws.readyPromise_].promise;
//   }
// }
