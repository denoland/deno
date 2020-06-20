// Copyright 2019 the Deno authors. All rights reserved. MIT license.
import { core } from "../core.ts";
import { close } from "./resources.ts";

export interface FsEvent {
  kind: "any" | "access" | "create" | "modify" | "remove";
  paths: string[];
}

class FsWatcher implements AsyncIterableIterator<FsEvent> {
  readonly rid: number;

  constructor(paths: string[], options: { recursive: boolean }) {
    const { recursive } = options;
    this.rid = core.dispatchJson.sendSync("op_fs_events_open", {
      recursive,
      paths,
    });
  }

  next(): Promise<IteratorResult<FsEvent>> {
    return core.dispatchJson.sendAsync("op_fs_events_poll", {
      rid: this.rid,
    });
  }

  return(value?: FsEvent): Promise<IteratorResult<FsEvent>> {
    close(this.rid);
    return Promise.resolve({ value, done: true });
  }

  [Symbol.asyncIterator](): AsyncIterableIterator<FsEvent> {
    return this;
  }
}

export function watchFs(
  paths: string | string[],
  options = { recursive: true }
): AsyncIterableIterator<FsEvent> {
  return new FsWatcher(Array.isArray(paths) ? paths : [paths], options);
}
