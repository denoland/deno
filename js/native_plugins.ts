// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
// TODO rename this file to js/plugins.ts
import { sendSync, sendAnySync } from "./dispatch";
import * as msg from "gen/cli/msg_generated";
import * as flatbuffers from "./flatbuffers";
import { assert } from "./util";
import { build } from "./build";

export type PluginCallReturn = Uint8Array | undefined;

function pluginCallInner(baseRes: msg.Base): PluginCallReturn {
  assert(baseRes != null);
  assert(
    msg.Any.PluginCallRes === baseRes!.innerType(),
    `base.innerType() unexpectedly is ${baseRes!.innerType()}`
  );
  const res = new msg.PluginCallRes();
  assert(baseRes!.inner(res) != null);

  const dataArray = res.dataArray();
  if (dataArray === null) {
    return undefined;
  }
  return dataArray;
}

function pluginCall(
  rid: number,
  data: Uint8Array,
  zeroCopy?: ArrayBufferView
): Promise<PluginCallReturn> | PluginCallReturn {
  const builder = flatbuffers.createBuilder();
  const data_ = builder.createString(data);
  const inner = msg.PluginCall.createPluginCall(builder, rid, data_);
  const response = sendAnySync(builder, msg.Any.PluginCall, inner, zeroCopy);
  if (response instanceof Promise) {
    return new Promise(
      async (resolve): Promise<void> => {
        resolve(pluginCallInner(await response));
      }
    );
  } else {
    if (response != null) {
      return pluginCallInner(response);
    } else {
      return undefined;
    }
  }
}

function pluginSym(libId: number, name: string): number {
  const builder = flatbuffers.createBuilder();
  const name_ = builder.createString(name);
  const inner = msg.PluginSym.createPluginSym(builder, libId, name_);
  const baseRes = sendSync(builder, msg.Any.PluginSym, inner);
  assert(baseRes != null);
  assert(
    msg.Any.PluginSymRes === baseRes!.innerType(),
    `base.innerType() unexpectedly is ${baseRes!.innerType()}`
  );
  const res = new msg.PluginSymRes();
  assert(baseRes!.inner(res) != null);
  return res.rid();
}

export interface PluginOp {
  dispatch(
    data: Uint8Array,
    zeroCopy?: ArrayBufferView
  ): Promise<PluginCallReturn> | PluginCallReturn;
}

// A loaded dynamic lib function.
// Loaded functions will need to loaded and addressed by unique identifiers
// for performance, since loading a function from a library for every call
// would likely be the limiting factor for many use cases.
// @internal
class PluginOpImpl implements PluginOp {
  private readonly rid: number;

  constructor(dlId: number, name: string) {
    this.rid = pluginSym(dlId, name);
  }

  dispatch(
    data: Uint8Array,
    zeroCopy?: ArrayBufferView
  ): Promise<PluginCallReturn> | PluginCallReturn {
    return pluginCall(this.rid, data, zeroCopy);
  }
}

// TODO Rename to pluginOpen
function dlOpen(filename: string): number {
  const builder = flatbuffers.createBuilder();
  const filename_ = builder.createString(filename);
  const inner = msg.PluginOpen.createPluginOpen(builder, filename_);
  const baseRes = sendSync(builder, msg.Any.PluginOpen, inner);
  assert(baseRes != null);
  assert(
    msg.Any.PluginOpenRes === baseRes!.innerType(),
    `base.innerType() unexpectedly is ${baseRes!.innerType()}`
  );
  const res = new msg.PluginOpenRes();
  assert(baseRes!.inner(res) != null);
  return res.rid();
}

export interface Plugin {
  loadOp(name: string): PluginOp;
}

// A loaded dynamic lib.
// Dynamic libraries need to remain loaded into memory on the rust side
// ,and then be addressed by their unique identifier to avoid loading
// the same library multiple times.
export class PluginImpl implements Plugin {
  // unique resource identifier for the loaded dynamic lib rust side
  private readonly rid: number;
  private readonly fnMap: Map<string, PluginOp> = new Map();

  // @internal
  constructor(libraryPath: string) {
    this.rid = dlOpen(libraryPath);
  }

  loadOp(name: string): PluginOp {
    const cachedFn = this.fnMap.get(name);
    if (cachedFn) {
      return cachedFn;
    } else {
      const dlFn = new PluginOpImpl(this.rid, name);
      this.fnMap.set(name, dlFn);
      return dlFn;
    }
  }
}

export function openPlugin(filename: string): Plugin {
  return new PluginImpl(filename);
}

export type PluginFilePrefix = "lib" | "";

const pluginFilePrefix = ((): PluginFilePrefix => {
  switch (build.os) {
    case "linux":
    case "mac":
      return "lib";
    case "win":
    default:
      return "";
  }
})();

export type PluginFileExtension = "so" | "dylib" | "dll";

const pluginFileExtension = ((): PluginFileExtension => {
  switch (build.os) {
    case "linux":
      return "so";
    case "mac":
      return "dylib";
    case "win":
      return "dll";
  }
})();

export function pluginFilename(filenameBase: string): string {
  return pluginFilePrefix + filenameBase + "." + pluginFileExtension;
}
