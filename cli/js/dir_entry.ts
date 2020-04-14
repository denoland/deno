// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { StatResponse } from "./ops/fs/stat.ts";
import { FileInfo, FileInfoImpl } from "./file_info.ts";
import { assert } from "./util.ts";

export interface DirEntry extends FileInfo {
  name: string;
}

// @internal
export class DirEntryImpl extends FileInfoImpl implements DirEntry {
  name: string;

  constructor (res: StatResponse) {
    super(res);
    assert(res.name != null);
    this.name = res.name;
  }
}
