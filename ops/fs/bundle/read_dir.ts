// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { JsonOp } from "deno_dispatch_json";
import { opNamespace } from "./namespace.ts";
import { FileInfo, FileInfoImpl } from "./file_info.ts";
import { StatResponse } from "./stat.ts";

const OP_READ_DIR = new JsonOp(opNamespace, "readDir");

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
  return res(OP_READ_DIR.sendSync({ path }));
}

/** Reads the directory given by path and returns a list of file info.
 *
 *       const files = await Deno.readDir("/");
 */
export async function readDir(path: string): Promise<FileInfo[]> {
  return res(await OP_READ_DIR.sendAsync({ path }));
}
