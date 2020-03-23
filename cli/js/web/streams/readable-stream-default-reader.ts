// Forked from https://github.com/stardazed/sd-streams/tree/8928cf04b035fd02fb1340b7eb541c76be37e546
// Copyright (c) 2018-Present by Arthur Langereis - @zenmumbler MIT

import * as rs from "./readable-internals.ts";
import * as shared from "./shared-internals.ts";

export class ReadableStreamDefaultReader<OutputType>
  implements rs.SDReadableStreamReader<OutputType> {
  [rs.closedPromise_]: shared.ControlledPromise<void>;
  [rs.ownerReadableStream_]: rs.SDReadableStream<OutputType> | undefined;
  [rs.readRequests_]: Array<rs.ReadRequest<IteratorResult<OutputType>>>;

  constructor(stream: rs.SDReadableStream<OutputType>) {
    if (!rs.isReadableStream(stream)) {
      throw new TypeError();
    }
    if (rs.isReadableStreamLocked(stream)) {
      throw new TypeError("The stream is locked.");
    }
    rs.readableStreamReaderGenericInitialize(this, stream);
    this[rs.readRequests_] = [];
  }

  get closed(): Promise<void> {
    if (!rs.isReadableStreamDefaultReader(this)) {
      return Promise.reject(new TypeError());
    }
    return this[rs.closedPromise_].promise;
  }

  cancel(reason: shared.ErrorResult): Promise<void> {
    if (!rs.isReadableStreamDefaultReader(this)) {
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

  read(): Promise<IteratorResult<OutputType | undefined>> {
    if (!rs.isReadableStreamDefaultReader(this)) {
      return Promise.reject(new TypeError());
    }
    if (this[rs.ownerReadableStream_] === undefined) {
      return Promise.reject(
        new TypeError("Reader is not associated with a stream")
      );
    }
    return rs.readableStreamDefaultReaderRead(this, true);
  }

  releaseLock(): void {
    if (!rs.isReadableStreamDefaultReader(this)) {
      throw new TypeError();
    }
    if (this[rs.ownerReadableStream_] === undefined) {
      return;
    }
    if (this[rs.readRequests_].length !== 0) {
      throw new TypeError("Cannot release a stream with pending read requests");
    }
    rs.readableStreamReaderGenericRelease(this);
  }
}
