// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { PartialReadError } from "../io/buf_reader.ts";
import type { Reader } from "../types.d.ts";

export interface TarInfo {
  fileMode?: number;
  mtime?: number;
  uid?: number;
  gid?: number;
  owner?: string;
  group?: string;
  type?: string;
}

export interface TarOptions extends TarInfo {
  /**
   * Filepath of the file to append to the archive
   */
  filePath?: string;

  /**
   * A Reader of any arbitrary content to append to the archive
   */
  reader?: Reader;

  /**
   * Size of the content to be appended.  This is only required
   * when passing a reader to the archive.
   */
  contentSize?: number;
}

export interface TarMeta extends TarInfo {
  fileName: string;
  fileSize?: number;
}

export enum FileTypes {
  "file" = 0,
  "link" = 1,
  "symlink" = 2,
  "character-device" = 3,
  "block-device" = 4,
  "directory" = 5,
  "fifo" = 6,
  "contiguous-file" = 7,
}

export const HEADER_LENGTH = 512;

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

export const ustarStructure: Array<{ field: string; length: number }> = [
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

export async function readBlock(
  reader: Reader,
  p: Uint8Array,
): Promise<number | null> {
  let bytesRead = 0;
  while (bytesRead < p.length) {
    const rr = await reader.read(p.subarray(bytesRead));
    if (rr === null) {
      if (bytesRead === 0) {
        return null;
      } else {
        throw new PartialReadError();
      }
    }
    bytesRead += rr;
  }
  return bytesRead;
}
