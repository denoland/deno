import { EventEmitter } from "ext:deno_node/events.ts";
import { Buffer } from "ext:deno_node/buffer.ts";
import { notImplemented } from "ext:deno_node/_utils.ts";

class FileHandle extends EventEmitter {
  readonly fd: number;
  constructor() {
  }

  // TODO implement this. https://github.com/nodejs/node/blob/959142a4652f7b6e90743be874fe9c065ed31d99/lib/internal/fs/promises.js#L173
  read(buffer: Buffer, offset: string, length: number, position?: number | null): void {
    notImplemented("not implemented FileHandle.read")
  }
}
