// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "../dispatch_json.ts";
import { FileInfo, StatResponse, parseFileInfo } from "./stat.ts";

export interface DirEntry extends FileInfo {
  name: string;
}

interface ReadDirResponse {
  entries: StatResponse[];
}

function res(response: ReadDirResponse): DirEntry[] {
  return response.entries.map(
    (statRes: StatResponse): DirEntry => {
      return { ...parseFileInfo(statRes), name: statRes.name! };
    }
  );
}

export function readdirSync(path: string): Iterable<DirEntry> {
  return res(sendSync("op_read_dir", { path }))[Symbol.iterator]();
}

export function readdir(path: string): AsyncIterable<DirEntry> {
  const array = sendAsync("op_read_dir", { path }).then(res);
  return {
    async *[Symbol.asyncIterator](): AsyncIterableIterator<DirEntry> {
      yield* await array;
    },
  };
}
