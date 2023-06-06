// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { EventEmitter } from "ext:deno_node/events.ts";
import { Buffer } from "ext:deno_node/buffer.ts";
import { promises, read } from "ext:deno_node/fs.ts";
import type { Buffer } from "ext:deno_node/buffer.ts";
import {
  BinaryOptionsArgument,
  FileOptionsArgument,
  ReadOptions,
  TextOptionsArgument,
} from "ext:deno_node/_fs/_fs_common.ts";

interface FileReadResult {
  bytesRead: number;
  buffer: Buffer;
}

export class FileHandle extends EventEmitter {
  #rid: number;
  constructor(rid: number) {
    super();
    this.rid = rid;
  }

  get fd() {
    return this.rid;
  }

  read(
    buffer: Buffer,
    offset?: number,
    length?: number,
    position?: number | null,
  ): Promise<FileReadResult>;
  read(options?: ReadOptions): Promise<FileReadResult>;
  read(
    bufferOrOpt: Buffer | ReadOptions,
    offset?: number,
    length?: number,
    position?: number | null,
  ): Promise<FileReadResult> {
    if (bufferOrOpt instanceof Buffer) {
      return new Promise((resolve, reject) => {
        read(
          this.fd,
          bufferOrOpt,
          offset,
          length,
          position,
          (err, bytesRead, buffer) => {
            if (err) reject(err);
            else resolve({ buffer: buffer, bytesRead: bytesRead });
          },
        );
      });
    } else {
      return new Promise((resolve, reject) => {
        read(this.fd, bufferOrOpt, (err, bytesRead, buffer) => {
          if (err) reject(err);
          else resolve({ buffer: buffer, bytesRead: bytesRead });
        });
      });
    }
  }

  readFile(
    opt?: TextOptionsArgument | BinaryOptionsArgument | FileOptionsArgument,
  ): Promise<string | Buffer> {
    return promises.readFile(this, opt);
  }

  close(): Promise<void> {
    // Note that Deno.close is not async
    return Promise.resolve(Deno.close(this.fd));
  }
}

export default {
  FileHandle,
};
