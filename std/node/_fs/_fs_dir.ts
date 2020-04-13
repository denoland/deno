import Dirent from "./_fs_dirent.ts";

export default class Dir {
  private dirPath: string | Uint8Array;
  private files: Dirent[] = [];
  private filesReadComplete = false;

  constructor(path: string | Uint8Array) {
    this.dirPath = path;
  }

  get path(): string {
    if (this.dirPath instanceof Uint8Array) {
      return new TextDecoder().decode(this.dirPath);
    }
    return this.dirPath;
  }

  /**
   * NOTE: Deno doesn't provide an interface to the filesystem like readdir
   * where each call to readdir returns the next file.  This function simulates this
   * behaviour by fetching all the entries on the first call, putting them on a stack
   * and then popping them off the stack one at a time.
   *
   * TODO: Rework this implementation once https://github.com/denoland/deno/issues/4218
   * is resolved.
   */
  read(callback?: Function): Promise<Dirent | null> {
    return new Promise(async (resolve, reject) => {
      try {
        if (this.initializationOfDirectoryFilesIsRequired()) {
          const denoFiles: Deno.FileInfo[] = await Deno.readdir(this.path);
          this.files = denoFiles.map((file) => new Dirent(file));
        }
        const nextFile = this.files.pop();
        if (nextFile) {
          resolve(nextFile);
          this.filesReadComplete = this.files.length === 0;
        } else {
          this.filesReadComplete = true;
          resolve(null);
        }
        if (callback) {
          callback(null, !nextFile ? null : nextFile);
        }
      } catch (err) {
        if (callback) {
          callback(err, null);
        }
        reject(err);
      }
    });
  }

  readSync(): Dirent | null {
    if (this.initializationOfDirectoryFilesIsRequired()) {
      this.files.push(
        ...Deno.readdirSync(this.path).map((file) => new Dirent(file))
      );
    }
    const dirent: Dirent | undefined = this.files.pop();
    this.filesReadComplete = this.files.length === 0;

    return !dirent ? null : dirent;
  }

  private initializationOfDirectoryFilesIsRequired(): boolean {
    return this.files.length === 0 && !this.filesReadComplete;
  }

  /**
   * Unlike Node, Deno does not require managing resource ids for reading
   * directories, and therefore does not need to close directories when
   * finished reading.
   */
  close(callback?: Function): Promise<void> {
    return new Promise((resolve, reject) => {
      try {
        if (callback) {
          callback(null);
        }
        resolve();
      } catch (err) {
        if (callback) {
          callback(err);
        }
        reject(err);
      }
    });
  }

  /**
   * Unlike Node, Deno does not require managing resource ids for reading
   * directories, and therefore does not need to close directories when
   * finished reading
   */
  closeSync(): void {
    //No op
  }

  async *[Symbol.asyncIterator](): AsyncIterableIterator<Dirent> {
    try {
      while (true) {
        const dirent: Dirent | null = await this.read();
        if (dirent === null) {
          break;
        }
        yield dirent;
      }
    } finally {
      await this.close();
    }
  }
}
