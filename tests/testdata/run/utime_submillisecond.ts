// Copyright 2018-2026 the Deno authors. MIT license.

import { lutimes, lutimesSync, utimes, utimesSync } from "node:fs";
import {
  lutimes as lutimesPromise,
  utimes as utimesPromise,
} from "node:fs/promises";

const [api, kind] = Deno.args;
const times = {
  number: [1_600_000_000.7654321, 1_700_000_000.1234567],
  date: [new Date(1_600_000_000_765), new Date(1_700_000_000_123)],
  "edge-date": [new Date(-2_007), new Date(1_001)],
} as const;
const [atime, mtime] = times[kind as keyof typeof times];

const isLink = api.startsWith("link_");
const target = "file.txt";
const path = isLink ? `${api}-${kind}` : target;
Deno.writeTextFileSync(target, "");
if (isLink) {
  Deno.utimeSync(target, 100, 200);
}

switch (api) {
  case "deno_sync":
    Deno.utimeSync(path, atime, mtime);
    break;
  case "deno_async":
    await Deno.utime(path, atime, mtime);
    break;
  case "file_sync": {
    using file = Deno.openSync(path, { write: true });
    file.utimeSync(atime, mtime);
    break;
  }
  case "file_async": {
    using file = await Deno.open(path, { write: true });
    await file.utime(atime, mtime);
    break;
  }
  case "node_sync":
  case "link_sync":
    (isLink ? lutimesSync : utimesSync)(path, atime, mtime);
    break;
  case "node_callback":
  case "link_callback":
    await new Promise<void>((resolve, reject) => {
      (isLink ? lutimes : utimes)(path, atime, mtime, (error) => {
        if (error) reject(error);
        else resolve();
      });
    });
    break;
  case "node_promise":
  case "link_promise":
    await (isLink ? lutimesPromise : utimesPromise)(path, atime, mtime);
    break;
  default:
    throw new Error(`Unknown API: ${api}`);
}
