// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
import { ModuleInfo } from "./types";
import { pubInternal, sub } from "./dispatch";
import { main as pb } from "./msg.pb";
import { assert, generateUniqueIdOnMap } from "./util";

export function initOS(): void {
  sub("os", (payload: Uint8Array) => {
    const msg = pb.Msg.decode(payload);
    assert(msg.command === pb.Msg.Command.READ_FILE_RES);
    const id = msg.readFileResId;
    const r = readFileRequests.get(id);
    assert(r != null, `Couldn't find ReadFileRequest id ${id}`);

    r.onMsg(msg);
  });
}

export function exit(exitCode = 0): void {
  pubInternal("os", {
    command: pb.Msg.Command.EXIT,
    exitCode
  });
}

export function codeFetch(
  moduleSpecifier: string,
  containingFile: string
): ModuleInfo {
  const res = pubInternal("os", {
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
  pubInternal("os", {
    command: pb.Msg.Command.CODE_CACHE,
    codeCacheFilename: filename,
    codeCacheSourceCode: sourceCode,
    codeCacheOutputCode: outputCode
  });
}

export function readFileSync(filename: string): Uint8Array {
  const res = pubInternal("os", {
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
  pubInternal("os", {
    command: pb.Msg.Command.WRITE_FILE_SYNC,
    writeFileSyncFilename: filename,
    writeFileSyncData: data,
    writeFileSyncPerm: perm
  });
}

const readFileRequests = new Map<number, ReadFileContext>();

class ReadFileContext {
  private readonly id: number;
  constructor(public filename: string) {
    this.id = generateUniqueIdOnMap(readFileRequests);
    readFileRequests.set(this.id, this);
  }

  onMsg(msg: pb.Msg) {
    if (msg.error !== null && msg.error !== "") {
      //throw new Error(msg.error)
      this.onError(new Error(msg.error));
      return;
    }
    this.onData(msg.readFileResData);

    this.destroy();
  }

  onError: (error: Error) => void;
  onData: (data: Uint8Array) => void;

  destroy() {
    readFileRequests.delete(this.id);
  }

  start() {
    pubInternal("os", {
      command: pb.Msg.Command.READ_FILE,
      readFileReqFilename: this.filename,
      readFileReqId: this.id,
    });
  }
}

export function readFile(filename: string): Promise<Uint8Array> {
  const ctx = new ReadFileContext(filename);
  return new Promise((resolve, reject) => {
    ctx.onData = (data: Uint8Array) => {
      resolve(data);
    };
    ctx.onError = (error: Error) => {
      reject(error);
    };
    ctx.start();
  });
}
