// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as msg from "gen/cli/msg_generated";
import { sendSync } from "./dispatch";
import * as flatbuffers from "./flatbuffers";
import { assert } from "./util";

export class Crypto {
  getRandomValues(): number {
    const builder = flatbuffers.createBuilder();
    const inner = msg.RandomValues.createRandomValues(builder);
    const baseRes = sendSync(builder, msg.Any.RandomValues, inner)!;
    assert(msg.Any.RandomValuesRes === baseRes.innerType());
    const res = new msg.RandomValuesRes();
    assert(baseRes.inner(res) != null);
    return res.val();
  }
}
