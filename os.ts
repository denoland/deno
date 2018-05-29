// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
import { ModuleInfo } from "./types";
import { sendMsg } from "./dispatch";
import { main as pb } from "./msg.pb";
import { assert } from "./util";

export function exit(exitCode = 0): void {
  sendMsg("os", {
    command: pb.Msg.Command.EXIT,
    exitCode
  });
}

export function codeFetch(
  moduleSpecifier: string,
  containingFile: string
): ModuleInfo {
  const res = sendMsg("os", {
    command: pb.Msg.Command.CODE_FETCH,
    codeFetchModuleSpecifier: moduleSpecifier,
    codeFetchContainingFile: containingFile
  });
  assert(res.command === pb.Msg.Command.CODE_FETCH_RES);
  return {
    moduleName: res.codeFetchResModuleName,
    filename: res.codeFetchResFilename,
    sourceCode: res.codeFetchResSourceCode,
    outputCode: res.codeFetchResOutputCode
  };
}

export function codeCache(
  filename: string,
  sourceCode: string,
  outputCode: string
): void {
  sendMsg("os", {
    command: pb.Msg.Command.CODE_CACHE,
    codeCacheFilename: filename,
    codeCacheSourceCode: sourceCode,
    codeCacheOutputCode: outputCode
  });
}

export function readFileSync(filename: string): Uint8Array {
  const res = sendMsg("os", {
    command: pb.Msg.Command.READ_FILE_SYNC,
    readFileSyncFilename: filename
  });
  return res.readFileSyncData;
}

export function writeFileSync(
  filename: string,
  data: Uint8Array,
  perm: number
): void {
  sendMsg("os", {
    command: pb.Msg.Command.WRITE_FILE_SYNC,
    writeFileSyncFilename: filename,
    writeFileSyncData: data,
    writeFileSyncPerm: perm
  });
}
