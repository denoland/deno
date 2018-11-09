// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as msg from "gen/msg_generated";
import * as flatbuffers from "./flatbuffers";
import { assert } from "./util";
import * as dispatch from "./dispatch";

export type ResourceMap = { [rid: number]: string };

/** Returns a map of open _file like_ resource ids along with their string
 * representation.
 */
export function resources(): ResourceMap {
  const builder = flatbuffers.createBuilder();
  msg.Resources.startResources(builder);
  const inner = msg.Resource.endResource(builder);
  const baseRes = dispatch.sendSync(builder, msg.Any.Resources, inner);
  assert(baseRes !== null);
  assert(msg.Any.ResourcesRes === baseRes!.innerType());
  const res = new msg.ResourcesRes();
  assert(baseRes!.inner(res) !== null);

  const resources = {} as ResourceMap;

  for (let i = 0; i < res.resourcesLength(); i++) {
    const item = res.resources(i)!;
    resources[item.rid()!] = item.repr()!;
  }

  return resources;
}
