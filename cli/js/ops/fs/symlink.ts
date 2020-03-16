// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "../dispatch_json.ts";
import * as util from "../../util.ts";
import { build } from "../../build.ts";

export function symlinkSync(
  oldname: string,
  newname: string,
  type?: string
): void {
  if (build.os === "win" && type) {
    return util.notImplemented();
  }
  sendSync("op_symlink", { oldname, newname });
}

export async function symlink(
  oldname: string,
  newname: string,
  type?: string
): Promise<void> {
  if (build.os === "win" && type) {
    return util.notImplemented();
  }
  await sendAsync("op_symlink", { oldname, newname });
}
