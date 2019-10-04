// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "./dispatch_json.ts";
import * as dispatch from "./dispatch.ts";
import { FileInfo, FileInfoImpl } from "./file_info.ts";
import { StatResponse } from "./stat.ts";

interface ReadDirResponse {
  entries: StatResponse[];
}

function res(response: ReadDirResponse): FileInfo[] {
  return response.entries.map(
    (statRes: StatResponse): FileInfo => {
      return new FileInfoImpl(statRes);
    }
  );
}

/** Reads the directory given by path and returns a list of file info
 * synchronously.
 *
 *       const files = Deno.readDirSync("/");
 */
export function readDirSync(path: string): FileInfo[] {
  return res(sendSync(dispatch.OP_READ_DIR, { path }));
}

/** Reads the directory given by path and returns a list of file info.
 *
 *       const files = await Deno.readDir("/");
 */
export async function readDir(path: string): Promise<FileInfo[]> {
  return res(await sendAsync(dispatch.OP_READ_DIR, { path }));
}
