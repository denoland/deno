import { log } from "./util.ts";
import * as fs from "./ops/fs.ts";

// eslint-disable-next-line @typescript-eslint/no-unused-vars
function main(): void {
  // eslint-disable-next-line
  (globalThis as any).fs = fs;
  log("hello world");
}
