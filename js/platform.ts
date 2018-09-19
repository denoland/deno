// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as fbs from "gen/msg_generated";
import { flatbuffers } from "flatbuffers";
import { assert } from "./util";
import * as dispatch from "./dispatch";

export interface PlatformInfo {
  os: string;
  family: string;
}

let cachedPlatformInfo: PlatformInfo | null = null;

/**
 * Retrieves information (os, os family) about the current
 * platform (synchronously).
 *
 *     import { platform } from "deno";
 *     const plat = platform();
 *     // On Linux, would print "linux" and "unix"
 *     // On Windows, would print "windows" and "windows"
 *     console.log(plat.os, plat.family)
 */
export function platform(): PlatformInfo {
  if (!cachedPlatformInfo) {
    cachedPlatformInfo = Object.freeze(res(dispatch.sendSync(...req())));
  }
  return cachedPlatformInfo!;
}

function req(): [flatbuffers.Builder, fbs.Any, flatbuffers.Offset] {
  const builder = new flatbuffers.Builder();
  fbs.Platform.startPlatform(builder);
  const msg = fbs.Platform.endPlatform(builder);
  return [builder, fbs.Any.Platform, msg];
}

function res(baseRes: null | fbs.Base): PlatformInfo {
  assert(baseRes != null);
  assert(fbs.Any.PlatformRes === baseRes!.msgType());
  const msg = new fbs.PlatformRes();
  assert(baseRes!.msg(msg) != null);
  const os = msg.os()!;
  const family = msg.family()!;
  assert(os != null);
  assert(family != null);
  return { os, family };
}
