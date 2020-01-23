// Copyright 2019 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "./dispatch_json.ts";
import * as dispatch from "./dispatch.ts";
import { Closer } from "./io.ts";
import { ErrorKind, DenoError } from "./errors.ts";

export interface FsWatcherEvent {
  event: string;
  source?: string;
  destination?: string;
}

export type FsWatcher = AsyncIterableIterator<FsWatcherEvent> & Closer;

export interface WatchOptions {
  recursive?: boolean;
}

class FsWatcherImpl implements FsWatcher {
  readonly rid: number;
  private closed = false;

  constructor(paths: string[], options: WatchOptions) {
    const { recursive = false } = options;
    this.rid = sendSync(dispatch.OP_WATCH, { recursive, paths });
  }

  async next(): Promise<IteratorResult<FsWatcherEvent>> {
    if (this.closed) {
      return { value: undefined, done: true };
    }

    try {
      const value = await sendAsync(dispatch.OP_POLL_WATCH, {
        rid: this.rid
      });
      // If empty value is returned that means that watcher was closed
      if (!value.event) {
        return { value: undefined, done: true };
      }
      return { value: value.event as FsWatcherEvent, done: false };
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

  [Symbol.asyncIterator](): AsyncIterableIterator<FsWatcherEvent> {
    return this;
  }
}

export function watch(
  paths: string | string[],
  options: WatchOptions = {}
): FsWatcher {
  return new FsWatcherImpl(Array.isArray(paths) ? paths : [paths], options);
}
