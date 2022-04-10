import { copy, EventEmitter } from "../deps.ts";
import GzipWriter from "./writer_gzip.ts";
import GunzipWriter from "./writer_gunzip.ts";

export class GzipStream extends EventEmitter {
  constructor() {
    super();
  }

  async compress(src: string, dest: string): Promise<void> {
    // reader
    const stat = await Deno.stat(src);
    const size = stat.size;
    const reader = await Deno.open(src, {
      read: true,
    });
    // writer
    const writer = new GzipWriter(dest, {
      onceSize: size > 50 * 1024 * 1024 ? 1024 * 1024 : 512 * 1024,
    });
    await writer.setup(
      src,
      stat.mtime ? Math.round(stat.mtime.getTime() / 1000) : 0,
    );
    writer.on("bytesWritten", (bytesWritten: number) => {
      const progress = (100 * bytesWritten / size).toFixed(2) + "%";
      this.emit("progress", progress);
    });

    /** 1: use Deno.copy */
    await copy(reader, writer, {
      bufSize: 1024 * 1024,
    });

    /** 2: not use Deno.copy */
    // let readed: number | null;
    // const n = 16384; //16kb
    // while (true) {
    //   const p: Uint8Array = new Uint8Array(n);
    //   readed = await reader.read(p);
    //   if (readed === null) break;
    //   if (readed < n) {
    //     await writer.write(p.subarray(0, readed));
    //     break;
    //   } else {
    //     await writer.write(p);
    //   }
    // }

    writer.close();
    reader.close();
  }

  async uncompress(src: string, dest: string): Promise<void> {
    // reader
    const size = (await Deno.stat(src)).size;
    const reader = await Deno.open(src, {
      read: true,
    });
    // writer
    const writer = new GunzipWriter(dest, {
      onceSize: size > 50 * 1024 * 1024 ? 1024 * 1024 : 512 * 1024,
    });
    await writer.setup();
    writer.on("bytesWritten", (bytesWritten: number) => {
      const progress = (100 * bytesWritten / size).toFixed(2) + "%";
      this.emit("progress", progress);
    });
    // write
    await copy(reader, writer, {
      bufSize: 1024 * 1024,
    });
    // close
    writer.close();
    reader.close();
  }
}
