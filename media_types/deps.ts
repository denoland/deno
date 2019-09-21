// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

export { extname } from "../fs/path.ts";

interface DB {
  [mediaType: string]: {
    source?: string;
    compressible?: boolean;
    charset?: string;
    extensions?: string[];
  };
}

import _db from "./db_c50e0d1.json";
export const db: DB = _db;
