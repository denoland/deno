import { EventEmitter } from "ext:deno_node/events.ts";
import { Buffer } from "ext:deno_node/buffer.ts";
import { promises } from "ext:deno_node/fs.ts";

export class FileHandle extends EventEmitter {
  readonly rid: number;
  constructor(rid: number) {
    super()
    this.rid = rid;
  }

  get fd() {
    return this.rid;
  }

  readFile(options: { encoding?: string | null; flag?: string; } | null = null): Promise<Buffer> {
    return promises.readFile(this, options);
  }
}

export default {
  FileHandle,
}
