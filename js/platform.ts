// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as fbs from "gen/msg_generated";
import { flatbuffers } from "flatbuffers";
import { assert } from "./util";
import * as dispatch from "./dispatch";

export interface PlatformInfo {
  os: string;
  family: string;
  endian: string;
}

/**
 * Retrieves information about the current platform synchronously.
 *
 *     import { platformSync } from "deno";
 *     const platform = deno.platformSync();
 */
export function platformSync(): PlatformInfo {
  return res(dispatch.sendSync(...req()));
}

/**
 * Creates a new directory with the specified path and permission.
 *
 *     import { platform } from "deno";
 *     const platform = await deno.platform();
 */
export async function platform(): Promise<PlatformInfo> {
  return res(await dispatch.sendAsync(...req()));
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
  const endian = msg.endian()!;
  assert(os != null);
  assert(family != null);
  assert(endian != null);
  return { os, family, endian };
}
