// Copyright 2019 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "./dispatch_json.ts";
import * as dispatch from "./dispatch.ts";
import { close } from "./files.ts";

export interface FsEvent {
  kind: "any" | "access" | "create" | "modify" | "remove";
  paths: string[];
}

class FsEvents implements AsyncIterableIterator<FsEvent> {
  readonly rid: number;

  constructor(paths: string[], options: { recursive: boolean }) {
    const { recursive } = options;
    this.rid = sendSync(dispatch.OP_FS_EVENTS_OPEN, { recursive, paths });
  }

  async next(): Promise<IteratorResult<FsEvent>> {
    return await sendAsync(dispatch.OP_FS_EVENTS_POLL, {
      rid: this.rid
    });
  }

  async return(value?: FsEvent): Promise<IteratorResult<FsEvent>> {
    close(this.rid);
    return { value, done: true };
  }

  [Symbol.asyncIterator](): AsyncIterableIterator<FsEvent> {
    return this;
  }
}

export function fsEvents(
  paths: string | string[],
  options = { recursive: true }
): AsyncIterableIterator<FsEvent> {
  return new FsEvents(Array.isArray(paths) ? paths : [paths], options);
}
