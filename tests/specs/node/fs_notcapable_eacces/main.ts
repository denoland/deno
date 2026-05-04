// Test that NotCapable errors are converted to EACCES in node:fs
import { constants, mkdirSync, openSync, writeFileSync } from "node:fs";

function testOp(name: string, fn: () => void): void {
  try {
    fn();
    console.log(`${name}: ERROR - should have thrown`);
  } catch (e: unknown) {
    const err = e as NodeJS.ErrnoException;
    if (err.code === "EACCES") {
      console.log(`${name}: EACCES`);
    } else {
      console.log(`${name}: WRONG - got ${err.code} instead of EACCES`);
    }
  }
}

testOp("writeFileSync", () => writeFileSync("/tmp/blocked.txt", "test"));
testOp(
  "openSync",
  () => openSync("/tmp/blocked.txt", constants.O_WRONLY | constants.O_CREAT),
);
testOp("mkdirSync", () => mkdirSync("/tmp/blocked_dir"));
