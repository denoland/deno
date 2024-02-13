// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
/*!
 * Ported and modified from: https://github.com/beatgammit/tar-js and
 * licensed as:
 *
 * (The MIT License)
 *
 * Copyright (c) 2011 T. Jameson Little
 * Copyright (c) 2019 Jun Kato
 * Copyright (c) 2018-2023 the Deno authors
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to deal
 * in the Software without restriction, including without limitation the rights
 * to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
 * copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in
 * all copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
 * THE SOFTWARE.
 */

import {
  FileTypes,
  type TarInfo,
  type TarMeta,
  type TarOptions,
  ustarStructure,
} from "./_common.ts";
import type { Reader } from "../types.d.ts";
import { MultiReader } from "../io/multi_reader.ts";
import { Buffer } from "../io/buffer.ts";
import { assert } from "../assert/assert.ts";
import { HEADER_LENGTH } from "./_common.ts";

export { type TarInfo, type TarMeta, type TarOptions };

const USTAR_MAGIC_HEADER = "ustar\u000000";

/**
 * Simple file reader
 */
class FileReader implements Reader {
  #file?: Deno.FsFile;

  constructor(private filePath: string) {}

  public async read(p: Uint8Array): Promise<number | null> {
    if (!this.#file) {
      this.#file = await Deno.open(this.filePath, { read: true });
    }
    const res = await this.#file.read(p);
    if (res === null) {
      this.#file.close();
      this.#file = undefined;
    }
    return res;
  }
}

/**
 * Initialize Uint8Array of the specified length filled with 0
 * @param length
 */
function clean(length: number): Uint8Array {
  const buffer = new Uint8Array(length);
  return buffer;
}

function pad(num: number, bytes: number, base = 8): string {
  const numString = num.toString(base);
  return "000000000000".slice(numString.length + 12 - bytes) + numString;
}

/**
 * Create header for a file in a tar archive
 */
function formatHeader(data: TarData): Uint8Array {
  const encoder = new TextEncoder();
  const buffer = clean(HEADER_LENGTH);
  let offset = 0;
  ustarStructure.forEach(function (value) {
    const entry = encoder.encode(data[value.field as keyof TarData] || "");
    buffer.set(entry, offset);
    offset += value.length; // space it out with nulls
  });
  return buffer;
}

export interface TarData {
  fileName?: string;
  fileNamePrefix?: string;
  fileMode?: string;
  uid?: string;
  gid?: string;
  fileSize?: string;
  mtime?: string;
  checksum?: string;
  type?: string;
  ustar?: string;
  owner?: string;
  group?: string;
}

export interface TarDataWithSource extends TarData {
  /**
   * file to read
   */
  filePath?: string;
  /**
   * buffer to read
   */
  reader?: Reader;
}

/**
 * ### Overview
 * A class to create a tar archive.  Tar archives allow for storing multiple files in a
 * single file (called an archive, or sometimes a tarball).  These archives typically
 * have the '.tar' extension.
 *
 * ### Usage
 * The workflow is to create a Tar instance, append files to it, and then write the
 * tar archive to the filesystem (or other output stream).  See the worked example
 * below for details.
 *
 * ### Compression
 * Tar archives are not compressed by default.  If you want to compress the archive,
 * you may compress the tar archive after creation, but this capability is not provided
 * here.
 *
 * ### File format and limitations
 *
 * The ustar file format is used for creating the archive file.
 * While this format is compatible with most tar readers,
 * the format has several limitations, including:
 * * Files must be smaller than 8GiB
 * * Filenames (including path) must be shorter than 256 characters
 * * Filenames (including path) cannot contain non-ASCII characters
 * * Sparse files are not supported
 *
 * @example
 * ```ts
 * import { Tar } from "https://deno.land/std@$STD_VERSION/archive/tar.ts";
 * import { Buffer } from "https://deno.land/std@$STD_VERSION/io/buffer.ts";
 * import { copy } from "https://deno.land/std@$STD_VERSION/streams/copy.ts";
 *
 * const tar = new Tar();
 *
 * // Now that we've created our tar, let's add some files to it:
 *
 * const content = new TextEncoder().encode("Some arbitrary content");
 * await tar.append("deno.txt", {
 *   reader: new Buffer(content),
 *   contentSize: content.byteLength,
 * });
 *
 * // This file is sourced from the filesystem (and renamed in the archive)
 * await tar.append("filename_in_archive.txt", {
 *   filePath: "./filename_on_filesystem.txt",
 * });
 *
 * // Now let's write the tar (with it's two files) to the filesystem
 * // use tar.getReader() to read the contents.
 *
 * const writer = await Deno.open("./out.tar", { write: true, create: true });
 * await copy(tar.getReader(), writer);
 * writer.close();
 * ```
 */
export class Tar {
  data: TarDataWithSource[];

  constructor() {
    this.data = [];
  }

  /**
   * Append a file or reader of arbitrary content to this tar archive. Directories
   * appended to the archive append only the directory itself to the archive, not
   * its contents.  To add a directory and its contents, recursively append the
   * directory's contents.  Directories and subdirectories will be created automatically
   * in the archive as required.
   *
   * @param filenameInArchive file name of the content in the archive
   *                 e.g., test.txt; use slash for directory separators
   * @param source details of the source of the content including the
   *               reference to the content itself and potentially any
   *               related metadata.
   */
  async append(filenameInArchive: string, source: TarOptions) {
    if (typeof filenameInArchive !== "string") {
      throw new Error("file name not specified");
    }
    let fileName = filenameInArchive;

    /**
     * Ustar format has a limitation of file name length.  Specifically:
     * 1. File names can contain at most 255 bytes.
     * 2. File names longer than 100 bytes must be split at a directory separator in two parts,
     * the first being at most 155 bytes long. So, in most cases file names must be a bit shorter
     * than 255 bytes.
     */
    // separate file name into two parts if needed
    let fileNamePrefix: string | undefined;
    if (fileName.length > 100) {
      let i = fileName.length;
      while (i >= 0) {
        i = fileName.lastIndexOf("/", i);
        if (i <= 155) {
          fileNamePrefix = fileName.slice(0, i);
          fileName = fileName.slice(i + 1);
          break;
        }
        i--;
      }
      const errMsg =
        "ustar format does not allow a long file name (length of [file name" +
        "prefix] + / + [file name] must be shorter than 256 bytes)";
      if (i < 0 || fileName.length > 100) {
        throw new Error(errMsg);
      } else {
        assert(fileNamePrefix !== undefined);
        if (fileNamePrefix.length > 155) {
          throw new Error(errMsg);
        }
      }
    }

    source = source || {};

    // set meta data
    let info: Deno.FileInfo | undefined;
    if (source.filePath) {
      info = await Deno.stat(source.filePath);
      if (info.isDirectory) {
        info.size = 0;
        source.reader = new Buffer();
      }
    }

    const mode = source.fileMode || (info && info.mode) ||
      parseInt("777", 8) & 0xfff /* 511 */;
    const mtime = Math.floor(
      source.mtime ?? (info?.mtime ?? new Date()).valueOf() / 1000,
    );
    const uid = source.uid || 0;
    const gid = source.gid || 0;

    if (typeof source.owner === "string" && source.owner.length >= 32) {
      throw new Error(
        "ustar format does not allow owner name length >= 32 bytes",
      );
    }
    if (typeof source.group === "string" && source.group.length >= 32) {
      throw new Error(
        "ustar format does not allow group name length >= 32 bytes",
      );
    }

    const fileSize = info?.size ?? source.contentSize;
    assert(fileSize !== undefined, "fileSize must be set");

    const type = source.type
      ? FileTypes[source.type as keyof typeof FileTypes]
      : (info?.isDirectory ? FileTypes.directory : FileTypes.file);
    const tarData: TarDataWithSource = {
      fileName,
      fileNamePrefix,
      fileMode: pad(mode, 7),
      uid: pad(uid, 7),
      gid: pad(gid, 7),
      fileSize: pad(fileSize, 11),
      mtime: pad(mtime, 11),
      checksum: "        ",
      type: type.toString(),
      ustar: USTAR_MAGIC_HEADER,
      owner: source.owner || "",
      group: source.group || "",
      filePath: source.filePath,
      reader: source.reader,
    };

    // calculate the checksum
    let checksum = 0;
    const encoder = new TextEncoder();
    Object.keys(tarData)
      .filter((key): boolean => ["filePath", "reader"].indexOf(key) < 0)
      .forEach(function (key) {
        checksum += encoder
          .encode(tarData[key as keyof TarData])
          .reduce((p, c): number => p + c, 0);
      });

    tarData.checksum = pad(checksum, 6) + "\u0000 ";
    this.data.push(tarData);
  }

  /**
   * Get a Reader instance for this tar archive.
   */
  getReader(): Reader {
    const readers: Reader[] = [];
    this.data.forEach((tarData) => {
      let { reader } = tarData;
      const { filePath } = tarData;
      const headerArr = formatHeader(tarData);
      readers.push(new Buffer(headerArr));
      if (!reader) {
        assert(filePath !== undefined);
        reader = new FileReader(filePath);
      }
      readers.push(reader);

      // to the nearest multiple of recordSize
      assert(tarData.fileSize !== undefined, "fileSize must be set");
      readers.push(
        new Buffer(
          clean(
            HEADER_LENGTH -
              (parseInt(tarData.fileSize, 8) % HEADER_LENGTH || HEADER_LENGTH),
          ),
        ),
      );
    });

    // append 2 empty records
    readers.push(new Buffer(clean(HEADER_LENGTH * 2)));
    return new MultiReader(readers);
  }
}
