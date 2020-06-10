/**
 * Ported and modified from: https://github.com/beatgammit/tar-js and
 * licensed as:
 *
 * (The MIT License)
 *
 * Copyright (c) 2011 T. Jameson Little
 * Copyright (c) 2019 Jun Kato
 * Copyright (c) 2020 the Deno authors
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
import { MultiReader } from "../io/readers.ts";
import { BufReader } from "../io/bufio.ts";
import { assert } from "../_util/assert.ts";

const recordSize = 512;
const ustar = "ustar\u000000";

/**
 * Simple file reader
 */
class FileReader implements Deno.Reader {
  private file?: Deno.File;

  constructor(private filePath: string) {}

  public async read(p: Uint8Array): Promise<number | null> {
    if (!this.file) {
      this.file = await Deno.open(this.filePath, { read: true });
    }
    const res = await Deno.read(this.file.rid, p);
    if (res === null) {
      Deno.close(this.file.rid);
      this.file = undefined;
    }
    return res;
  }
}

/**
 * Remove the trailing null codes
 * @param buffer
 */
function trim(buffer: Uint8Array): Uint8Array {
  const index = buffer.findIndex((v): boolean => v === 0);
  if (index < 0) return buffer;
  return buffer.subarray(0, index);
}

/**
 * Initialize Uint8Array of the specified length filled with 0
 * @param length
 */
function clean(length: number): Uint8Array {
  const buffer = new Uint8Array(length);
  buffer.fill(0, 0, length - 1);
  return buffer;
}

function pad(num: number, bytes: number, base?: number): string {
  const numString = num.toString(base || 8);
  return "000000000000".substr(numString.length + 12 - bytes) + numString;
}

/*
struct posix_header {           // byte offset
	char name[100];               //   0
	char mode[8];                 // 100
	char uid[8];                  // 108
	char gid[8];                  // 116
	char size[12];                // 124
	char mtime[12];               // 136
	char chksum[8];               // 148
	char typeflag;                // 156
	char linkname[100];           // 157
	char magic[6];                // 257
	char version[2];              // 263
	char uname[32];               // 265
	char gname[32];               // 297
	char devmajor[8];             // 329
	char devminor[8];             // 337
	char prefix[155];             // 345
                                // 500
};
*/

const ustarStructure: Array<{ field: string; length: number }> = [
  {
    field: "fileName",
    length: 100,
  },
  {
    field: "fileMode",
    length: 8,
  },
  {
    field: "uid",
    length: 8,
  },
  {
    field: "gid",
    length: 8,
  },
  {
    field: "fileSize",
    length: 12,
  },
  {
    field: "mtime",
    length: 12,
  },
  {
    field: "checksum",
    length: 8,
  },
  {
    field: "type",
    length: 1,
  },
  {
    field: "linkName",
    length: 100,
  },
  {
    field: "ustar",
    length: 8,
  },
  {
    field: "owner",
    length: 32,
  },
  {
    field: "group",
    length: 32,
  },
  {
    field: "majorNumber",
    length: 8,
  },
  {
    field: "minorNumber",
    length: 8,
  },
  {
    field: "fileNamePrefix",
    length: 155,
  },
  {
    field: "padding",
    length: 12,
  },
];

/**
 * Create header for a file in a tar archive
 */
function formatHeader(data: TarData): Uint8Array {
  const encoder = new TextEncoder(),
    buffer = clean(512);
  let offset = 0;
  ustarStructure.forEach(function (value): void {
    const entry = encoder.encode(data[value.field as keyof TarData] || "");
    buffer.set(entry, offset);
    offset += value.length; // space it out with nulls
  });
  return buffer;
}

/**
 * Parse file header in a tar archive
 * @param length
 */
function parseHeader(buffer: Uint8Array): { [key: string]: Uint8Array } {
  const data: { [key: string]: Uint8Array } = {};
  let offset = 0;
  ustarStructure.forEach(function (value): void {
    const arr = buffer.subarray(offset, offset + value.length);
    data[value.field] = arr;
    offset += value.length;
  });
  return data;
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
  reader?: Deno.Reader;
}

export interface TarInfo {
  fileMode?: number;
  mtime?: number;
  uid?: number;
  gid?: number;
  owner?: string;
  group?: string;
}

export interface TarOptions extends TarInfo {
  /**
   * append file
   */
  filePath?: string;

  /**
   * append any arbitrary content
   */
  reader?: Deno.Reader;

  /**
   * size of the content to be appended
   */
  contentSize?: number;
}

export interface UntarOptions extends TarInfo {
  fileName: string;
}

/**
 * A class to create a tar archive
 */
export class Tar {
  data: TarDataWithSource[];

  constructor() {
    this.data = [];
  }

  /**
   * Append a file to this tar archive
   * @param fn file name
   *                 e.g., test.txt; use slash for directory separators
   * @param opts options
   */
  async append(fn: string, opts: TarOptions): Promise<void> {
    if (typeof fn !== "string") {
      throw new Error("file name not specified");
    }
    let fileName = fn;
    // separate file name into two parts if needed
    let fileNamePrefix: string | undefined;
    if (fileName.length > 100) {
      let i = fileName.length;
      while (i >= 0) {
        i = fileName.lastIndexOf("/", i);
        if (i <= 155) {
          fileNamePrefix = fileName.substr(0, i);
          fileName = fileName.substr(i + 1);
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
        assert(fileNamePrefix != null);
        if (fileNamePrefix.length > 155) {
          throw new Error(errMsg);
        }
      }
    }

    opts = opts || {};

    // set meta data
    let info: Deno.FileInfo | undefined;
    if (opts.filePath) {
      info = await Deno.stat(opts.filePath);
    }

    const mode =
        opts.fileMode || (info && info.mode) || parseInt("777", 8) & 0xfff,
      mtime = Math.floor(
        opts.mtime ?? (info?.mtime ?? new Date()).valueOf() / 1000
      ),
      uid = opts.uid || 0,
      gid = opts.gid || 0;
    if (typeof opts.owner === "string" && opts.owner.length >= 32) {
      throw new Error(
        "ustar format does not allow owner name length >= 32 bytes"
      );
    }
    if (typeof opts.group === "string" && opts.group.length >= 32) {
      throw new Error(
        "ustar format does not allow group name length >= 32 bytes"
      );
    }

    const fileSize = info?.size ?? opts.contentSize;
    assert(fileSize != null, "fileSize must be set");
    const tarData: TarDataWithSource = {
      fileName,
      fileNamePrefix,
      fileMode: pad(mode, 7),
      uid: pad(uid, 7),
      gid: pad(gid, 7),
      fileSize: pad(fileSize, 11),
      mtime: pad(mtime, 11),
      checksum: "        ",
      type: "0", // just a file
      ustar,
      owner: opts.owner || "",
      group: opts.group || "",
      filePath: opts.filePath,
      reader: opts.reader,
    };

    // calculate the checksum
    let checksum = 0;
    const encoder = new TextEncoder();
    Object.keys(tarData)
      .filter((key): boolean => ["filePath", "reader"].indexOf(key) < 0)
      .forEach(function (key): void {
        checksum += encoder
          .encode(tarData[key as keyof TarData])
          .reduce((p, c): number => p + c, 0);
      });

    tarData.checksum = pad(checksum, 6) + "\u0000 ";
    this.data.push(tarData);
  }

  /**
   * Get a Reader instance for this tar data
   */
  getReader(): Deno.Reader {
    const readers: Deno.Reader[] = [];
    this.data.forEach((tarData): void => {
      let { reader } = tarData;
      const { filePath } = tarData;
      const headerArr = formatHeader(tarData);
      readers.push(new Deno.Buffer(headerArr));
      if (!reader) {
        assert(filePath != null);
        reader = new FileReader(filePath);
      }
      readers.push(reader);

      // to the nearest multiple of recordSize
      assert(tarData.fileSize != null, "fileSize must be set");
      readers.push(
        new Deno.Buffer(
          clean(
            recordSize -
              (parseInt(tarData.fileSize, 8) % recordSize || recordSize)
          )
        )
      );
    });

    // append 2 empty records
    readers.push(new Deno.Buffer(clean(recordSize * 2)));
    return new MultiReader(...readers);
  }
}

/**
 * A class to create a tar archive
 */
export class Untar {
  reader: BufReader;
  block: Uint8Array;

  constructor(reader: Deno.Reader) {
    this.reader = new BufReader(reader);
    this.block = new Uint8Array(recordSize);
  }

  async extract(writer: Deno.Writer): Promise<UntarOptions> {
    await this.reader.readFull(this.block);
    const header = parseHeader(this.block);

    // calculate the checksum
    let checksum = 0;
    const encoder = new TextEncoder(),
      decoder = new TextDecoder("ascii");
    Object.keys(header)
      .filter((key): boolean => key !== "checksum")
      .forEach(function (key): void {
        checksum += header[key].reduce((p, c): number => p + c, 0);
      });
    checksum += encoder.encode("        ").reduce((p, c): number => p + c, 0);

    if (parseInt(decoder.decode(header.checksum), 8) !== checksum) {
      throw new Error("checksum error");
    }

    const magic = decoder.decode(header.ustar);
    if (magic !== ustar) {
      throw new Error(`unsupported archive format: ${magic}`);
    }

    // get meta data
    const meta: UntarOptions = {
      fileName: decoder.decode(trim(header.fileName)),
    };
    const fileNamePrefix = trim(header.fileNamePrefix);
    if (fileNamePrefix.byteLength > 0) {
      meta.fileName = decoder.decode(fileNamePrefix) + "/" + meta.fileName;
    }
    (["fileMode", "mtime", "uid", "gid"] as [
      "fileMode",
      "mtime",
      "uid",
      "gid"
    ]).forEach((key): void => {
      const arr = trim(header[key]);
      if (arr.byteLength > 0) {
        meta[key] = parseInt(decoder.decode(arr), 8);
      }
    });
    (["owner", "group"] as ["owner", "group"]).forEach((key): void => {
      const arr = trim(header[key]);
      if (arr.byteLength > 0) {
        meta[key] = decoder.decode(arr);
      }
    });

    // read the file content
    const len = parseInt(decoder.decode(header.fileSize), 8);
    let rest = len;
    while (rest > 0) {
      await this.reader.readFull(this.block);
      const arr = rest < recordSize ? this.block.subarray(0, rest) : this.block;
      await Deno.copy(new Deno.Buffer(arr), writer);
      rest -= recordSize;
    }

    return meta;
  }
}
