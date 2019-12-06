// Copyright 2019 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "./dispatch_json.ts";
import * as dispatch from "./dispatch.ts";
import { Closer } from "./io.ts";

export interface FsWatcherEvent {
  event: string;
  source?: string;
  destination?: string;
}

export type FsWatcher = AsyncIterableIterator<FsWatcherEvent> & Closer;

export interface WatchOptions {
  recursive?: boolean;
  debounce?: number;
}

class FsWatcherImpl implements FsWatcher {
  readonly rid: number;
  private closed: boolean = false;

  constructor(paths: string[], options: WatchOptions) {
    const { recursive = false, debounce = 500 } = options;
    this.rid = sendSync(dispatch.OP_WATCH, { recursive, paths, debounce });
  }

  async next(): Promise<IteratorResult<FsWatcherEvent>> {
    if (this.closed) {
      return { value: undefined, done: true };
    }
    const res: FsWatcherEvent = await sendAsync(dispatch.OP_POLL_WATCH, {
      rid: this.rid
    });
    return { value: res, done: res.event === "watcherClosed" };
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
