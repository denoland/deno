// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
/* eslint-disable @typescript-eslint/no-explicit-any */
import * as msg from "gen/cli/msg_generated";
import { sendSync } from "./dispatch";
import { decodeMessage } from "./workers";
import { assert } from "./util";
import * as flatbuffers from "./flatbuffers";

export function deps(): any {
  const builder = flatbuffers.createBuilder();
  const inner = msg.Deps.createDeps(builder);
  const baseRes = sendSync(builder, msg.Any.Deps, inner)!;
  assert(msg.Any.DepsRes === baseRes.innerType());
  const res = new msg.DepsRes();
  assert(baseRes.inner(res) != null);
  // TypeScript cannot track assertion above,
  const dataArray = res.dataArray();
  if (dataArray != null) {
    return decodeMessage(dataArray);
  } else {
    return null;
  }
}
