import process from "node:process";
import { readFileSync } from "node:fs";

process.nextTick(() => {
  readFileSync("/non/existent/file");
});
