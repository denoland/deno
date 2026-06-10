import process from "node:process";
import { readFileSync } from "node:fs";

process.nextTick(() => {
  try {
    readFileSync("/non/existent/file");
  } catch (error) {
    console.log(error);
  }
});
