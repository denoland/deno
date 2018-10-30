// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as msg from "gen/msg_generated";
import * as flatbuffers from "./flatbuffers";
import { assert } from "./util";
import * as dispatch from "./dispatch";

interface Resource {
  rid: number;
  repr: string;
}

export function resources(): Resource[] {
  return res(dispatch.sendSync(...req()));
}

function req(): [flatbuffers.Builder, msg.Any, flatbuffers.Offset] {
  const builder = flatbuffers.createBuilder();
  msg.Resources.startResources(builder);
  const inner = msg.Resource.endResource(builder);
  return [builder, msg.Any.Resources, inner];
}

function res(baseRes: null | msg.Base): Resource[] {
  assert(baseRes !== null);
  assert(msg.Any.ResourcesRes === baseRes!.innerType());
  const res = new msg.ResourcesRes();
  assert(baseRes!.inner(res) !== null);

  const resources: Resource[] = [];

  for (let i = 0; i < res.resourcesLength(); i++) {
    const item = res.resources(i)!;

    resources.push({
      rid: item.rid()!,
      repr: item.repr()!
    });
  }

  return resources.sort((a, b) => a.rid - b.rid);
}
