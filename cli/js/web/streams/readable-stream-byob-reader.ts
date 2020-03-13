// Forked from https://github.com/stardazed/sd-streams/tree/8928cf04b035fd02fb1340b7eb541c76be37e546
// Copyright (c) 2018-Present by Arthur Langereis - @zenmumbler MIT

import * as rs from "./readable-internals.ts";
import * as shared from "./shared-internals.ts";

export class SDReadableStreamBYOBReader
  implements rs.SDReadableStreamBYOBReader {
  [rs.closedPromise_]: shared.ControlledPromise<void>;
  [rs.ownerReadableStream_]: rs.SDReadableStream<ArrayBufferView> | undefined;
  [rs.readIntoRequests_]: Array<
    rs.ReadRequest<IteratorResult<ArrayBufferView>>
  >;

  constructor(stream: rs.SDReadableStream<ArrayBufferView>) {
    if (!rs.isReadableStream(stream)) {
      throw new TypeError();
    }
    if (
      !rs.isReadableByteStreamController(stream[rs.readableStreamController_])
    ) {
      throw new TypeError();
    }
    if (rs.isReadableStreamLocked(stream)) {
      throw new TypeError("The stream is locked.");
    }
    rs.readableStreamReaderGenericInitialize(this, stream);
    this[rs.readIntoRequests_] = [];
  }

  get closed(): Promise<void> {
    if (!rs.isReadableStreamBYOBReader(this)) {
      return Promise.reject(new TypeError());
    }
    return this[rs.closedPromise_].promise;
  }

  cancel(reason: shared.ErrorResult): Promise<void> {
    if (!rs.isReadableStreamBYOBReader(this)) {
      return Promise.reject(new TypeError());
    }
    const stream = this[rs.ownerReadableStream_];
    if (stream === undefined) {
      return Promise.reject(
        new TypeError("Reader is not associated with a stream")
      );
    }
    return rs.readableStreamCancel(stream, reason);
  }

  read(view: ArrayBufferView): Promise<IteratorResult<ArrayBufferView>> {
    if (!rs.isReadableStreamBYOBReader(this)) {
      return Promise.reject(new TypeError());
    }
    if (this[rs.ownerReadableStream_] === undefined) {
      return Promise.reject(
        new TypeError("Reader is not associated with a stream")
      );
    }
    if (!ArrayBuffer.isView(view)) {
      return Promise.reject(
        new TypeError("view argument must be a valid ArrayBufferView")
      );
    }
    // If ! IsDetachedBuffer(view.[[ViewedArrayBuffer]]) is true, return a promise rejected with a TypeError exception.
    if (view.byteLength === 0) {
      return Promise.reject(
        new TypeError("supplied buffer view must be > 0 bytes")
      );
    }
    return rs.readableStreamBYOBReaderRead(this, view, true);
  }

  releaseLock(): void {
    if (!rs.isReadableStreamBYOBReader(this)) {
      throw new TypeError();
    }
    if (this[rs.ownerReadableStream_] === undefined) {
      throw new TypeError("Reader is not associated with a stream");
    }
    if (this[rs.readIntoRequests_].length > 0) {
      throw new TypeError();
    }
    rs.readableStreamReaderGenericRelease(this);
  }
}
