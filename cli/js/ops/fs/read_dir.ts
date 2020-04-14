// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "../dispatch_json.ts";
import { DirEntry, DirEntryImpl } from "../../dir_entry.ts";
import { StatResponse } from "./stat.ts";

interface ReadDirResponse {
  entries: StatResponse[];
}

function res(response: ReadDirResponse): DirEntry[] {
  return response.entries.map(
    (statRes: StatResponse): DirEntry => {
      return new DirEntryImpl(statRes);
    }
  );
}

export function readdirSync(path: string): DirEntry[] {
  return res(sendSync("op_read_dir", { path }));
}

export async function readdir(path: string): Promise<DirEntry[]> {
  return res(await sendAsync("op_read_dir", { path }));
}
