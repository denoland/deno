// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as msg from "gen/cli/msg_generated";
import { sendSync } from "./dispatch";
import { decodeMessage } from "./workers";
import { assert } from "./util";
import * as flatbuffers from "./flatbuffers";

interface DepsArray {
  0: string;
  1: DepsArray[];
}

interface DepsObject {
  [index: string]: DepsObject;
}

export interface DepsOptions {
  returnObject?: boolean;
}

export function deps({ returnObject }: DepsOptions = {}):
  | DepsArray
  | DepsObject
  | null {
  const builder = flatbuffers.createBuilder();
  const inner = msg.Deps.createDeps(builder);
  const baseRes = sendSync(builder, msg.Any.Deps, inner)!;
  assert(msg.Any.DepsRes === baseRes.innerType());
  const res = new msg.DepsRes();
  assert(baseRes.inner(res) != null);
  // TypeScript cannot track assertion above,
  const dataArray = res.dataArray();
  if (dataArray != null) {
    const depsArray = decodeMessage(dataArray);
    return returnObject ? depsArrayToDepsObject(depsArray) : depsArray;
  } else {
    return null;
  }
}

function depsArrayToDepsObject(depsArray: DepsArray): DepsObject {
  return {
    [depsArray[0]]: depsArray[1].reduce(
      (depsObjects: object, subDepsArray: DepsArray) => ({
        ...depsObjects,
        ...depsArrayToDepsObject(subDepsArray)
      }),
      {}
    )
  };
}
