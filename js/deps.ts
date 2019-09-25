// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { sendAsync } from "./dispatch_json.ts";
import * as dispatch from "./dispatch.ts";

interface DepsRes {
  name: string;
  deps: [DepsRes]
}

export async function deps(url: string): Promise<DepsRes> {
  return await sendAsync(dispatch.OP_DEPS, { url });
}
