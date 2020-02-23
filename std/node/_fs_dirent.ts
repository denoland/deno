import { notImplemented } from "./_utils.ts";

export default class Dirent {
  constructor(private entry: Deno.FileInfo) {}

  isBlockDevice(): boolean {
    return this.entry.blocks != null;
  }

  isCharacterDevice(): boolean {
    return this.entry.blocks == null;
  }

  isDirectory(): boolean {
    return this.entry.isDirectory();
  }

  isFIFO(): boolean {
    notImplemented(
      "Deno does not yet support identification of FIFO named pipes"
    );
  }

  isFile(): boolean {
    return this.entry.isFile();
  }

  isSocket(): boolean {
    notImplemented("Deno does not yet support identification of sockets");
  }

  isSymbolicLink(): boolean {
    return this.entry.isSymlink();
  }

  get name(): string {
    return this.entry.name;
  }
}
