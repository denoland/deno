// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { EventEmitter } from "ext:deno_node/events.ts";
import { Buffer } from "ext:deno_node/buffer.ts";
import { promises, write } from "ext:deno_node/fs.ts";
import {
  BinaryOptionsArgument,
  FileOptionsArgument,
  TextOptionsArgument,
} from "ext:deno_node/_fs/_fs_common.ts";


interface WriteResult {
  bytesRead: number;
  buffer: Buffer;
}

type WriteOptions = {
  buffer: Buffer | Uint8Array;
  offset: number;
  length: number;
  position: number | null;
};

export class FileHandle extends EventEmitter {
  #rid: number;
  constructor(rid: number) {
    super();
    this.rid = rid;
  }

  get fd() {
    return this.rid;
  }

  readFile(
    opt?: TextOptionsArgument | BinaryOptionsArgument | FileOptionsArgument,
  ): Promise<string | Buffer> {
    return promises.readFile(this, opt);
  }

  write(buffer: Buffer, offset: number, length: number, position: number): promise<WriteResult> {
    return new Promise((resolve, reject) => {
      write(this.rid, buffer, offset, length, position, (err, bytesWritten, buffer) => {
        if (err) {
          reject(err);
        } else {
          resolve({ bytesRead: bytesWritten, buffer });
        }
      }
    };
  }

  close(): Promise<void> {
    // Note that Deno.close is not async
    return Promise.resolve(Deno.close(this.fd));
  }
}

export default {
  FileHandle,
};
