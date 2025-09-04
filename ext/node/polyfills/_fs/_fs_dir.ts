// Copyright 2018-2025 the Deno authors. MIT license.

import { primordials } from "ext:core/mod.js";
import {
  type Dirent,
  direntFromDeno,
} from "ext:deno_node/internal/fs/utils.mjs";
import { assert } from "ext:deno_node/_util/asserts.ts";
import { ERR_MISSING_ARGS } from "ext:deno_node/internal/errors.ts";
import { TextDecoder } from "ext:deno_web/08_text_encoding.js";

const {
  Promise,
  ObjectPrototypeIsPrototypeOf,
  Uint8ArrayPrototype,
  PromisePrototypeThen,
  SymbolAsyncIterator,
  ArrayIteratorPrototypeNext,
  AsyncGeneratorPrototypeNext,
  SymbolIterator,
} = primordials;

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
    if (ObjectPrototypeIsPrototypeOf(Uint8ArrayPrototype, this.#dirPath)) {
      return new TextDecoder().decode(this.#dirPath);
    }
    return this.#dirPath;
  }

  // deno-lint-ignore no-explicit-any
  read(callback?: (...args: any[]) => void): Promise<Dirent | null> {
    return new Promise((resolve, reject) => {
      if (!this.#asyncIterator) {
        this.#asyncIterator = Deno.readDir(this.path)[SymbolAsyncIterator]();
      }
      assert(this.#asyncIterator);
      PromisePrototypeThen(
        AsyncGeneratorPrototypeNext(this.#asyncIterator),
        (iteratorResult) => {
          resolve(
            iteratorResult.done ? null : direntFromDeno(iteratorResult.value),
          );
          if (callback) {
            callback(
              null,
              iteratorResult.done ? null : direntFromDeno(iteratorResult.value),
            );
          }
        },
        (err) => {
          if (callback) {
            callback(err);
          }
          reject(err);
        },
      );
    });
  }

  readSync(): Dirent | null {
    if (!this.#syncIterator) {
      this.#syncIterator = Deno.readDirSync(this.path)![SymbolIterator]();
    }

    const iteratorResult = ArrayIteratorPrototypeNext(this.#syncIterator);
    if (iteratorResult.done) {
      return null;
    } else {
      return direntFromDeno(iteratorResult.value);
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
