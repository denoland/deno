import { cpSync, mkdirSync, mkdtempSync, opendirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";

const prepare = (srcDir: string) => {
  for (let i = 1; i <= 5; i++) {
    const dirPath = path.join(srcDir, `dir${i}`);
    mkdirSync(dirPath);
  }
};

const execute = (sourceDir: string, destDir: string) => {
  let iter = 0;

  const filter = (_src: string, _dest: string) => {
    iter++;
    if (iter === 1) { // The parent directory.
      return true;
    } else if (iter === 2) {
      return 1; // Truthy value
    } else if (iter === 3) {
      return undefined; // Falsy value
    }
    throw new Error("Test error on iter 4");
  };

  // @ts-expect-error - Testing filter function with non-boolean return values.
  cpSync(sourceDir, destDir, { recursive: true, filter });
};

const assert = (destDir: string) => {
  const dir = opendirSync(destDir);
  let count = 0;
  for (let entry = dir.readSync(); entry !== null; entry = dir.readSync()) {
    count++;
  }
  dir.closeSync();
  if (count !== 1) {
    throw new Error(`Expected 1 directory to be copied, but found ${count}`);
  }
};

const main = () => {
  const srcDir = mkdtempSync(path.join(tmpdir(), "cp-src-"));
  const destDir = mkdtempSync(path.join(tmpdir(), "cp-dest-"));

  prepare(srcDir);
  try {
    execute(srcDir, destDir);
  } finally {
    assert(destDir);
    rmSync(srcDir, { recursive: true, force: true });
    rmSync(destDir, { recursive: true, force: true });
  }
};

main();
