// Copyright 2018-2026 the Deno authors. MIT license.

import { core, primordials } from "ext:core/mod.js";
import { type Dirent } from "ext:deno_node/internal/fs/utils.mjs";
const { ERR_MISSING_ARGS, ERR_DIR_CLOSED, ERR_INVALID_THIS } = core
  .loadExtScript(
    "ext:deno_node/internal/errors.ts",
  );
const { TextDecoder } = core.loadExtScript("ext:deno_web/08_text_encoding.js");
// Directory entries are produced natively in Rust (ext/node/ops/fs.rs); the
// whole listing is read up front and handed out one entry at a time.
import { op_node_fs_readdir, op_node_fs_readdir_sync } from "ext:core/ops";

const {
  Promise,
  PromiseResolve,
  ObjectPrototypeIsPrototypeOf,
  Uint8ArrayPrototype,
  PromisePrototypeThen,
  SymbolAsyncIterator,
  SymbolAsyncDispose,
  SymbolDispose,
} = primordials;

export default class Dir {
  #dirPath: string | Uint8Array;
  #entries: Dirent[] | null = null;
  #idx = 0;
  #closed = false;
  #recursive: boolean;

  constructor(path: string | Uint8Array, recursive = false) {
    if (!path) {
      throw new ERR_MISSING_ARGS("path");
    }
    this.#dirPath = path;
    this.#recursive = recursive;
  }

  get path(): string {
    // Match Node: invoking the getter on a non-Dir receiver (e.g. the
    // prototype) throws ERR_INVALID_THIS rather than a private-field error.
    // deno-lint-ignore prefer-primordials -- private-field brand check
    if (!(#dirPath in this)) {
      throw new ERR_INVALID_THIS("Dir");
    }
    if (ObjectPrototypeIsPrototypeOf(Uint8ArrayPrototype, this.#dirPath)) {
      return new TextDecoder().decode(this.#dirPath);
    }
    return this.#dirPath as string;
  }

  // deno-lint-ignore no-explicit-any
  read(callback?: (...args: any[]) => void): Promise<Dirent | null> {
    return new Promise((resolve, reject) => {
      if (this.#closed) {
        const err = new ERR_DIR_CLOSED();
        if (callback) {
          callback(err);
          resolve(null);
        } else {
          reject(err);
        }
        return;
      }
      const emit = (dirent: Dirent | null) => {
        if (callback) {
          callback(null, dirent);
        }
        resolve(dirent);
      };
      if (this.#entries === null) {
        PromisePrototypeThen(
          op_node_fs_readdir(this.path, this.#recursive, true),
          (entries: Dirent[]) => {
            this.#entries = entries;
            emit(this.#next());
          },
          (err) => {
            if (callback) {
              callback(err);
            }
            reject(err);
          },
        );
        return;
      }
      emit(this.#next());
    });
  }

  #next(): Dirent | null {
    if (this.#entries && this.#idx < this.#entries.length) {
      return this.#entries[this.#idx++];
    }
    return null;
  }

  readSync(): Dirent | null {
    if (this.#closed) {
      throw new ERR_DIR_CLOSED();
    }
    if (this.#entries === null) {
      this.#entries = op_node_fs_readdir_sync(
        this.path,
        this.#recursive,
        true,
      ) as unknown as Dirent[];
    }
    return this.#next();
  }

  /**
   * Unlike Node, Deno does not require managing resource ids for reading
   * directories, and therefore does not need to close directories when
   * finished reading.
   */
  // deno-lint-ignore no-explicit-any
  close(callback?: (...args: any[]) => void): Promise<void> {
    return new Promise((resolve, reject) => {
      // Match Node: closing an already-closed Dir is an error.
      if (this.#closed) {
        const err = new ERR_DIR_CLOSED();
        if (callback) {
          callback(err);
          resolve();
        } else {
          reject(err);
        }
        return;
      }
      this.#closed = true;
      if (callback) {
        callback(null);
      }
      resolve();
    });
  }

  /**
   * Unlike Node, Deno does not require managing resource ids for reading
   * directories, and therefore does not need to close directories when
   * finished reading
   */
  closeSync() {
    if (this.#closed) {
      throw new ERR_DIR_CLOSED();
    }
    this.#closed = true;
  }

  // Unlike explicit close()/closeSync(), the dispose protocol is idempotent:
  // repeated invocations must not throw (see node's file-handle-dispose test).
  [SymbolDispose]() {
    if (!this.#closed) this.closeSync();
  }

  [SymbolAsyncDispose](): Promise<void> {
    if (this.#closed) return PromiseResolve();
    return this.close();
  }

  async *[SymbolAsyncIterator](): AsyncIterableIterator<Dirent> {
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
