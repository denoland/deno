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

import _db from "./db_1.37.0.json";
export const db: DB = _db;
