// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertThrows,
  assertThrowsAsync,
} from "../testing/asserts.ts";
import { resolvePath } from "../fs/mod.ts";
import * as path from "../path/mod.ts";
import { copy, copySync } from "./copy.ts";
import { exists, existsSync } from "./exists.ts";
import { ensureDir, ensureDirSync } from "./ensure_dir.ts";
import { ensureFile, ensureFileSync } from "./ensure_file.ts";
import { ensureSymlink, ensureSymlinkSync } from "./ensure_symlink.ts";

const moduleDir = path.dirname(path.fromFileUrl(import.meta.url));
const testdataDir = resolvePath(moduleDir, "testdata");

function testCopy(
  name: string,
  cb: (tempDir: string) => Promise<void>,
  ignore = false,
): void {
  Deno.test({
    name,
    async fn(): Promise<void> {
      const tempDir = await Deno.makeTempDir({
        prefix: "deno_std_copy_async_test_",
      });
      await cb(tempDir);
      await Deno.remove(tempDir, { recursive: true });
    },
    ignore,
  });
}

function testCopySync(name: string, cb: (tempDir: string) => void): void {
  Deno.test({
    name,
    fn: (): void => {
      const tempDir = Deno.makeTempDirSync({
        prefix: "deno_std_copy_sync_test_",
      });
      cb(tempDir);
      Deno.removeSync(tempDir, { recursive: true });
    },
  });
}

testCopy(
  "[fs] copy file if it does no exist",
  async (tempDir: string): Promise<void> => {
    const srcFile = path.join(testdataDir, "copy_file_not_exists.txt");
    const destFile = path.join(tempDir, "copy_file_not_exists_1.txt");
    await assertThrowsAsync(
      async (): Promise<void> => {
        await copy(srcFile, destFile);
      },
    );
  },
);

testCopy(
  "[fs] copy if src and dest are the same paths",
  async (tempDir: string): Promise<void> => {
    const srcFile = path.join(tempDir, "copy_file_same.txt");
    const destFile = path.join(tempDir, "copy_file_same.txt");
    await assertThrowsAsync(
      async (): Promise<void> => {
        await copy(srcFile, destFile);
      },
      Error,
      "Source and destination cannot be the same.",
    );
  },
);

testCopy(
  "[fs] copy file",
  async (tempDir: string): Promise<void> => {
    const srcFile = path.join(testdataDir, "copy_file.txt");
    const destFile = path.join(tempDir, "copy_file_copy.txt");

    const srcContent = new TextDecoder().decode(await Deno.readFile(srcFile));

    assertEquals(
      await exists(srcFile),
      true,
      `source should exist before copy`,
    );
    assertEquals(
      await exists(destFile),
      false,
      "destination should not exist before copy",
    );

    await copy(srcFile, destFile);

    assertEquals(await exists(srcFile), true, "source should exist after copy");
    assertEquals(
      await exists(destFile),
      true,
      "destination should exist before copy",
    );

    const destContent = new TextDecoder().decode(await Deno.readFile(destFile));

    assertEquals(
      srcContent,
      destContent,
      "source and destination should have the same content",
    );

    // Copy again and it should throw an error.
    await assertThrowsAsync(
      async (): Promise<void> => {
        await copy(srcFile, destFile);
      },
      Error,
      `'${destFile}' already exists.`,
    );

    // Modify destination file.
    await Deno.writeFile(destFile, new TextEncoder().encode("txt copy"));

    assertEquals(
      new TextDecoder().decode(await Deno.readFile(destFile)),
      "txt copy",
    );

    // Copy again with overwrite option.
    await copy(srcFile, destFile, { overwrite: true });

    // Make sure the file has been overwritten.
    assertEquals(
      new TextDecoder().decode(await Deno.readFile(destFile)),
      "txt",
    );
  },
);

testCopy(
  "[fs] copy with preserve timestamps",
  async (tempDir: string): Promise<void> => {
    const srcFile = path.join(testdataDir, "copy_file.txt");
    const destFile = path.join(tempDir, "copy_file_copy.txt");

    const srcStatInfo = await Deno.stat(srcFile);

    assert(srcStatInfo.atime instanceof Date);
    assert(srcStatInfo.mtime instanceof Date);

    // Copy with overwrite and preserve timestamps options.
    await copy(srcFile, destFile, {
      overwrite: true,
      preserveTimestamps: true,
    });

    const destStatInfo = await Deno.stat(destFile);

    assert(destStatInfo.atime instanceof Date);
    assert(destStatInfo.mtime instanceof Date);
    assertEquals(destStatInfo.atime, srcStatInfo.atime);
    assertEquals(destStatInfo.mtime, srcStatInfo.mtime);
  },
);

testCopy(
  "[fs] copy directory to its subdirectory",
  async (tempDir: string): Promise<void> => {
    const srcDir = path.join(tempDir, "parent");
    const destDir = path.join(srcDir, "child");

    await ensureDir(srcDir);

    await assertThrowsAsync(
      async (): Promise<void> => {
        await copy(srcDir, destDir);
      },
      Error,
      `Cannot copy '${srcDir}' to a subdirectory of itself, '${destDir}'.`,
    );
  },
);

testCopy(
  "[fs] copy directory and destination exist and not a directory",
  async (tempDir: string): Promise<void> => {
    const srcDir = path.join(tempDir, "parent");
    const destDir = path.join(tempDir, "child.txt");

    await ensureDir(srcDir);
    await ensureFile(destDir);

    await assertThrowsAsync(
      async (): Promise<void> => {
        await copy(srcDir, destDir);
      },
      Error,
      `Cannot overwrite non-directory '${destDir}' with directory '${srcDir}'.`,
    );
  },
);

testCopy(
  "[fs] copy directory",
  async (tempDir: string): Promise<void> => {
    const srcDir = path.join(testdataDir, "copy_dir");
    const destDir = path.join(tempDir, "copy_dir");
    const srcFile = path.join(srcDir, "0.txt");
    const destFile = path.join(destDir, "0.txt");
    const srcNestFile = path.join(srcDir, "nest", "0.txt");
    const destNestFile = path.join(destDir, "nest", "0.txt");

    await copy(srcDir, destDir);

    assertEquals(await exists(destFile), true);
    assertEquals(await exists(destNestFile), true);

    // After copy. The source and destination should have the same content.
    assertEquals(
      new TextDecoder().decode(await Deno.readFile(srcFile)),
      new TextDecoder().decode(await Deno.readFile(destFile)),
    );
    assertEquals(
      new TextDecoder().decode(await Deno.readFile(srcNestFile)),
      new TextDecoder().decode(await Deno.readFile(destNestFile)),
    );

    // Copy again without overwrite option and it should throw an error.
    await assertThrowsAsync(
      async (): Promise<void> => {
        await copy(srcDir, destDir);
      },
      Error,
      `'${destDir}' already exists.`,
    );

    // Modify the file in the destination directory.
    await Deno.writeFile(destNestFile, new TextEncoder().encode("nest copy"));
    assertEquals(
      new TextDecoder().decode(await Deno.readFile(destNestFile)),
      "nest copy",
    );

    // Copy again with overwrite option.
    await copy(srcDir, destDir, { overwrite: true });

    // Make sure the file has been overwritten.
    assertEquals(
      new TextDecoder().decode(await Deno.readFile(destNestFile)),
      "nest",
    );
  },
);

testCopy(
  "[fs] copy symlink file",
  async (tempDir: string): Promise<void> => {
    const dir = path.join(testdataDir, "copy_dir_link_file");
    const srcLink = path.join(dir, "0.txt");
    const destLink = path.join(tempDir, "0_copy.txt");

    assert(
      (await Deno.lstat(srcLink)).isSymlink,
      `'${srcLink}' should be symlink type`,
    );

    await copy(srcLink, destLink);

    const statInfo = await Deno.lstat(destLink);

    assert(statInfo.isSymlink, `'${destLink}' should be symlink type`);
  },
);

testCopy(
  "[fs] copy symlink directory",
  async (tempDir: string): Promise<void> => {
    const srcDir = path.join(testdataDir, "copy_dir");
    const srcLink = path.join(tempDir, "copy_dir_link");
    const destLink = path.join(tempDir, "copy_dir_link_copy");

    await ensureSymlink(srcDir, srcLink);

    assert(
      (await Deno.lstat(srcLink)).isSymlink,
      `'${srcLink}' should be symlink type`,
    );

    await copy(srcLink, destLink);

    const statInfo = await Deno.lstat(destLink);

    assert(statInfo.isSymlink);
  },
);

testCopySync(
  "[fs] copy file synchronously if it does not exist",
  (tempDir: string): void => {
    const srcFile = path.join(testdataDir, "copy_file_not_exists_sync.txt");
    const destFile = path.join(tempDir, "copy_file_not_exists_1_sync.txt");
    assertThrows((): void => {
      copySync(srcFile, destFile);
    });
  },
);

testCopySync(
  "[fs] copy synchronously with preserve timestamps",
  (tempDir: string): void => {
    const srcFile = path.join(testdataDir, "copy_file.txt");
    const destFile = path.join(tempDir, "copy_file_copy.txt");

    const srcStatInfo = Deno.statSync(srcFile);

    assert(srcStatInfo.atime instanceof Date);
    assert(srcStatInfo.mtime instanceof Date);

    // Copy with overwrite and preserve timestamps options.
    copySync(srcFile, destFile, {
      overwrite: true,
      preserveTimestamps: true,
    });

    const destStatInfo = Deno.statSync(destFile);

    assert(destStatInfo.atime instanceof Date);
    assert(destStatInfo.mtime instanceof Date);
    // TODO: Activate test when https://github.com/denoland/deno/issues/2411
    // is fixed
    // assertEquals(destStatInfo.atime, srcStatInfo.atime);
    // assertEquals(destStatInfo.mtime, srcStatInfo.mtime);
  },
);

testCopySync(
  "[fs] copy synchronously if src and dest are the same paths",
  (): void => {
    const srcFile = path.join(testdataDir, "copy_file_same_sync.txt");
    assertThrows(
      (): void => {
        copySync(srcFile, srcFile);
      },
      Error,
      "Source and destination cannot be the same.",
    );
  },
);

testCopySync("[fs] copy file synchronously", (tempDir: string): void => {
  const srcFile = path.join(testdataDir, "copy_file.txt");
  const destFile = path.join(tempDir, "copy_file_copy_sync.txt");

  const srcContent = new TextDecoder().decode(Deno.readFileSync(srcFile));

  assertEquals(existsSync(srcFile), true);
  assertEquals(existsSync(destFile), false);

  copySync(srcFile, destFile);

  assertEquals(existsSync(srcFile), true);
  assertEquals(existsSync(destFile), true);

  const destContent = new TextDecoder().decode(Deno.readFileSync(destFile));

  assertEquals(srcContent, destContent);

  // Copy again without overwrite option and it should throw an error.
  assertThrows(
    (): void => {
      copySync(srcFile, destFile);
    },
    Error,
    `'${destFile}' already exists.`,
  );

  // Modify destination file.
  Deno.writeFileSync(destFile, new TextEncoder().encode("txt copy"));

  assertEquals(
    new TextDecoder().decode(Deno.readFileSync(destFile)),
    "txt copy",
  );

  // Copy again with overwrite option.
  copySync(srcFile, destFile, { overwrite: true });

  // Make sure the file has been overwritten.
  assertEquals(new TextDecoder().decode(Deno.readFileSync(destFile)), "txt");
});

testCopySync(
  "[fs] copy directory synchronously to its subdirectory",
  (tempDir: string): void => {
    const srcDir = path.join(tempDir, "parent");
    const destDir = path.join(srcDir, "child");

    ensureDirSync(srcDir);

    assertThrows(
      (): void => {
        copySync(srcDir, destDir);
      },
      Error,
      `Cannot copy '${srcDir}' to a subdirectory of itself, '${destDir}'.`,
    );
  },
);

testCopySync(
  "[fs] copy directory synchronously, and destination exist and not a " +
    "directory",
  (tempDir: string): void => {
    const srcDir = path.join(tempDir, "parent_sync");
    const destDir = path.join(tempDir, "child.txt");

    ensureDirSync(srcDir);
    ensureFileSync(destDir);

    assertThrows(
      (): void => {
        copySync(srcDir, destDir);
      },
      Error,
      `Cannot overwrite non-directory '${destDir}' with directory '${srcDir}'.`,
    );
  },
);

testCopySync("[fs] copy directory synchronously", (tempDir: string): void => {
  const srcDir = path.join(testdataDir, "copy_dir");
  const destDir = path.join(tempDir, "copy_dir_copy_sync");
  const srcFile = path.join(srcDir, "0.txt");
  const destFile = path.join(destDir, "0.txt");
  const srcNestFile = path.join(srcDir, "nest", "0.txt");
  const destNestFile = path.join(destDir, "nest", "0.txt");

  copySync(srcDir, destDir);

  assertEquals(existsSync(destFile), true);
  assertEquals(existsSync(destNestFile), true);

  // After copy. The source and destination should have the same content.
  assertEquals(
    new TextDecoder().decode(Deno.readFileSync(srcFile)),
    new TextDecoder().decode(Deno.readFileSync(destFile)),
  );
  assertEquals(
    new TextDecoder().decode(Deno.readFileSync(srcNestFile)),
    new TextDecoder().decode(Deno.readFileSync(destNestFile)),
  );

  // Copy again without overwrite option and it should throw an error.
  assertThrows(
    (): void => {
      copySync(srcDir, destDir);
    },
    Error,
    `'${destDir}' already exists.`,
  );

  // Modify the file in the destination directory.
  Deno.writeFileSync(destNestFile, new TextEncoder().encode("nest copy"));
  assertEquals(
    new TextDecoder().decode(Deno.readFileSync(destNestFile)),
    "nest copy",
  );

  // Copy again with overwrite option.
  copySync(srcDir, destDir, { overwrite: true });

  // Make sure the file has been overwritten.
  assertEquals(
    new TextDecoder().decode(Deno.readFileSync(destNestFile)),
    "nest",
  );
});

testCopySync(
  "[fs] copy symlink file synchronously",
  (tempDir: string): void => {
    const dir = path.join(testdataDir, "copy_dir_link_file");
    const srcLink = path.join(dir, "0.txt");
    const destLink = path.join(tempDir, "0_copy.txt");

    assert(
      Deno.lstatSync(srcLink).isSymlink,
      `'${srcLink}' should be symlink type`,
    );

    copySync(srcLink, destLink);

    const statInfo = Deno.lstatSync(destLink);

    assert(statInfo.isSymlink, `'${destLink}' should be symlink type`);
  },
);

testCopySync(
  "[fs] copy symlink directory synchronously",
  (tempDir: string): void => {
    const originDir = path.join(testdataDir, "copy_dir");
    const srcLink = path.join(tempDir, "copy_dir_link");
    const destLink = path.join(tempDir, "copy_dir_link_copy");

    ensureSymlinkSync(originDir, srcLink);

    assert(
      Deno.lstatSync(srcLink).isSymlink,
      `'${srcLink}' should be symlink type`,
    );

    copySync(srcLink, destLink);

    const statInfo = Deno.lstatSync(destLink);

    assert(statInfo.isSymlink);
  },
);
