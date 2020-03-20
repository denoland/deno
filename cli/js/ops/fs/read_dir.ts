// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "../dispatch_json.ts";
import { FileInfo, FileInfoImpl } from "../../file_info.ts";
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

export function readdirSync(path: string): FileInfo[] {
  return res(sendSync("op_read_dir", { path }));
}

export async function readdir(path: string): Promise<FileInfo[]> {
  return res(await sendAsync("op_read_dir", { path }));
}
