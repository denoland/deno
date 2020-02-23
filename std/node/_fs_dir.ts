import Dirent from "./_fs_dirent.ts";

export interface DirectoryOptions {
  encoding: string | "utf-8";
}

export default class Dir {
  private resourceId: number;
  private dirPath: string | Uint8Array;
  private options: DirectoryOptions = { encoding: "utf-8" };
  private files: Dirent[] = [];
  private filesReadComplete = false;

  constructor(
    resourceId: number,
    path: string | Uint8Array,
    options?: DirectoryOptions
  ) {
    this.resourceId = resourceId;
    this.dirPath = path;
    if (options) {
      this.options = options;
    }
  }

  get path(): string {
    if (this.dirPath instanceof Uint8Array) {
      return new TextDecoder().decode(this.dirPath);
    }
    return this.dirPath;
  }

  /**
   * NOTE: Deno doesn't currently provide an interface to the filesystem like readdir
   * where each call to readdir returns the next file.  This function simulates this
   * behaviour by fetching all the entries on the first call, putting them on a stack
   * and then poping them off the stack one at a time.
   */
  read(callback?: Function): Promise<Dirent | null> {
    this.validateDirectoryOpen();

    return new Promise(async (resolve, reject) => {
      try {
        if (this.initializationOfDirectoryFilesIsRequired()) {
          const denoFiles: Deno.FileInfo[] = await Deno.readDir(this.path);
          this.files = denoFiles.map(file => new Dirent(file));
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

  readSync(): Dirent {
    this.validateDirectoryOpen();
    if (this.initializationOfDirectoryFilesIsRequired()) {
      this.files = Deno.readDirSync(this.path).map(file => new Dirent(file));
    }
    const dirent: Dirent = this.files.pop();
    this.filesReadComplete = this.files.length === 0;

    return !dirent ? null : dirent;
  }

  private initializationOfDirectoryFilesIsRequired(): boolean {
    return this.files.length === 0 && !this.filesReadComplete;
  }

  close(callback?: Function): Promise<void> {
    this.validateDirectoryOpen();

    return new Promise((resolve, reject) => {
      try {
        Deno.close(this.resourceId);
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

  closeSync(): void {
    this.validateDirectoryOpen();
    Deno.close(this.resourceId);
  }

  async *entries(): AsyncIterableIterator<Dirent> {
    try {
      while (true) {
        const dirent: Dirent = await this.read();
        if (dirent === null) {
          break;
        }
        yield dirent;
      }
    } finally {
      await this.close();
    }
  }

  private validateDirectoryOpen(): void {
    if (!this.resourceId || !Deno.resources()[this.resourceId]) {
      throw new Error("Directory handle was closed");
    }
  }
}
