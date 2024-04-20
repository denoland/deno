// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import Dirent from "ext:deno_node/_fs/_fs_dirent.ts";
import { assert } from "ext:deno_node/_util/asserts.ts";
import { ERR_MISSING_ARGS } from "ext:deno_node/internal/errors.ts";
import { TextDecoder } from "ext:deno_web/08_text_encoding.js";

export default class Dir {
  #dirPath: string | Uint8Array;
  #syncIterator!: Iterator<Deno.DirEntry, undefined> | null;
  #asyncIterator!: AsyncIterator<Deno.DirEntry, undefined> | null;

  constructor(path: string | Uint8Array) {
    if (!path) {
      throw new ERR_MISSING_ARGS("path");
    }
    this.#dirPath = path;
  }

  get path(): string {
    if (this.#dirPath instanceof Uint8Array) {
      return new TextDecoder().decode(this.#dirPath);
    }
    return this.#dirPath;
  }

  // deno-lint-ignore no-explicit-any
  read(callback?: (...args: any[]) => void): Promise<Dirent | null> {
    return new Promise((resolve, reject) => {
      if (!this.#asyncIterator) {
        this.#asyncIterator = Deno.readDir(this.path)[Symbol.asyncIterator]();
      }
      assert(this.#asyncIterator);
      this.#asyncIterator
        .next()
        .then((iteratorResult) => {
          resolve(
            iteratorResult.done ? null : new Dirent(iteratorResult.value),
          );
          if (callback) {
            callback(
              null,
              iteratorResult.done ? null : new Dirent(iteratorResult.value),
            );
          }
        }, (err) => {
          if (callback) {
            callback(err);
          }
          reject(err);
        });
    });
  }

  readSync(): Dirent | null {
    if (!this.#syncIterator) {
      this.#syncIterator = Deno.readDirSync(this.path)![Symbol.iterator]();
    }

    const iteratorResult = this.#syncIterator.next();
    if (iteratorResult.done) {
      return null;
    } else {
      return new Dirent(iteratorResult.value);
    }
  }

  /**
   * Unlike Node, Deno does not require managing resource ids for reading
   * directories, and therefore does not need to close directories when
   * finished reading.
   */
  // deno-lint-ignore no-explicit-any
  close(callback?: (...args: any[]) => void): Promise<void> {
    return new Promise((resolve) => {
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
