// Copyright 2019 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "./dispatch_json.ts";
import { close } from "./resources.ts";

export interface FsEvent {
  kind: "any" | "access" | "create" | "modify" | "remove";
  paths: string[];
}

class FsEvents implements AsyncIterableIterator<FsEvent> {
  readonly rid: number;

  constructor(paths: string[], options: { recursive: boolean }) {
    const { recursive } = options;
    this.rid = sendSync("op_fs_events_open", { recursive, paths });
  }

  async next(): Promise<IteratorResult<FsEvent>> {
    return await sendAsync("op_fs_events_poll", {
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
