// Copyright 2019 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "./dispatch_json.ts";
import * as dispatch from "./dispatch.ts";
import { Closer } from "./io.ts";
import { ErrorKind, DenoError } from "./errors.ts";

export interface FsEvent {
  kind: "any" | "access" | "create" | "modify" | "remove";
  paths: string[];
}

export type FsWatcher = AsyncIterableIterator<FsEvent> & Closer;

export interface FsWatchOptions {
  recursive?: boolean;
}

class FsWatcherImpl implements FsWatcher {
  readonly rid: number;
  private closed = false;

  constructor(paths: string[], options: FsWatchOptions) {
    const { recursive = false } = options;
    this.rid = sendSync(dispatch.OP_FS_WATCH_OPEN, { recursive, paths });
  }

  async next(): Promise<IteratorResult<FsEvent>> {
    if (this.closed) {
      return { value: undefined, done: true };
    }

    try {
      return await sendAsync(dispatch.OP_FS_WATCH_POLL, {
        rid: this.rid
      });
    } catch (e) {
      if (e instanceof DenoError && e.kind == ErrorKind.BadResource) {
        return { value: undefined, done: true };
      } else {
        throw e;
      }
    }
  }

  close(): void {
    if (!this.closed) {
      sendSync(dispatch.OP_CLOSE, { rid: this.rid });
    }
    this.closed = true;
  }

  [Symbol.asyncIterator](): AsyncIterableIterator<FsEvent> {
    return this;
  }
}

export function watch(
  paths: string | string[],
  options: FsWatchOptions = {}
): FsWatcher {
  return new FsWatcherImpl(Array.isArray(paths) ? paths : [paths], options);
}
