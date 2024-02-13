// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../assert/mod.ts";
import * as path from "../path/mod.ts";
import { exists, existsSync } from "./exists.ts";

Deno.test("exists() returns false for a non-existent path", async function () {
  const tempDirPath = await Deno.makeTempDir();
  try {
    assertEquals(await exists(path.join(tempDirPath, "not_exists")), false);
  } finally {
    await Deno.remove(tempDirPath, { recursive: true });
  }
});

Deno.test("existsSync() returns false for a non-existent path", function () {
  const tempDirPath = Deno.makeTempDirSync();
  try {
    assertEquals(existsSync(path.join(tempDirPath, "not_exists")), false);
  } finally {
    Deno.removeSync(tempDirPath, { recursive: true });
  }
});

Deno.test("exists() returns true for an existing file", async function () {
  const tempDirPath = await Deno.makeTempDir();
  const tempFilePath = path.join(tempDirPath, "0.ts");
  const tempFile = await Deno.create(tempFilePath);
  try {
    assertEquals(await exists(tempFilePath), true);
    assertEquals(await exists(tempFilePath, {}), true);
    assertEquals(
      await exists(tempFilePath, {
        isDirectory: true,
      }),
      false,
    );
    assertEquals(
      await exists(tempFilePath, {
        isFile: true,
      }),
      true,
    );
    if (Deno.build.os !== "windows") {
      // TODO(martin-braun): include mode check for Windows tests when chmod is ported to NT
      await Deno.chmod(tempFilePath, 0o000);
      assertEquals(
        await exists(tempFilePath, {
          isReadable: true,
        }),
        false,
      );
    }
  } finally {
    if (Deno.build.os !== "windows") {
      await Deno.chmod(tempFilePath, 0o644);
    }
    tempFile.close();
    await Deno.remove(tempDirPath, { recursive: true });
  }
});

Deno.test("exists() returns true for an existing file symlink", async function () {
  const tempDirPath = await Deno.makeTempDir();
  const tempFilePath = path.join(tempDirPath, "0.ts");
  const tempLinkFilePath = path.join(tempDirPath, "0-link.ts");
  const tempFile = await Deno.create(tempFilePath);
  try {
    await Deno.symlink(tempFilePath, tempLinkFilePath);
    assertEquals(await exists(tempLinkFilePath), true);
    assertEquals(await exists(tempLinkFilePath, {}), true);
    assertEquals(
      await exists(tempLinkFilePath, {
        isDirectory: true,
      }),
      false,
    );
    assertEquals(
      await exists(tempLinkFilePath, {
        isFile: true,
      }),
      true,
    );
    if (Deno.build.os !== "windows") {
      // TODO(martin-braun): include mode check for Windows tests when chmod is ported to NT
      await Deno.chmod(tempFilePath, 0o000);
      assertEquals(
        await exists(tempLinkFilePath, {
          isReadable: true,
        }),
        false,
      );
      // TODO(martin-braun): test unreadable link when Rust's nix::sys::stat::fchmodat has been implemented
    }
  } finally {
    if (Deno.build.os !== "windows") {
      await Deno.chmod(tempFilePath, 0o644);
    }
    tempFile.close();
    await Deno.remove(tempDirPath, { recursive: true });
  }
});

Deno.test("existsSync() returns true for an existing file", function () {
  const tempDirPath = Deno.makeTempDirSync();
  const tempFilePath = path.join(tempDirPath, "0.ts");
  const tempFile = Deno.createSync(tempFilePath);
  try {
    assertEquals(existsSync(tempFilePath), true);
    assertEquals(existsSync(tempFilePath, {}), true);
    assertEquals(
      existsSync(tempFilePath, {
        isDirectory: true,
      }),
      false,
    );
    assertEquals(
      existsSync(tempFilePath, {
        isFile: true,
      }),
      true,
    );
    if (Deno.build.os !== "windows") {
      // TODO(martin-braun): include mode check for Windows tests when chmod is ported to NT
      Deno.chmodSync(tempFilePath, 0o000);
      assertEquals(
        existsSync(tempFilePath, {
          isReadable: true,
        }),
        false,
      );
    }
  } finally {
    if (Deno.build.os !== "windows") {
      Deno.chmodSync(tempFilePath, 0o644);
    }
    tempFile.close();
    Deno.removeSync(tempDirPath, { recursive: true });
  }
});

Deno.test("existsSync() returns true for an existing file symlink", function () {
  const tempDirPath = Deno.makeTempDirSync();
  const tempFilePath = path.join(tempDirPath, "0.ts");
  const tempLinkFilePath = path.join(tempDirPath, "0-link.ts");
  const tempFile = Deno.createSync(tempFilePath);
  try {
    Deno.symlinkSync(tempFilePath, tempLinkFilePath);
    assertEquals(existsSync(tempLinkFilePath), true);
    assertEquals(existsSync(tempLinkFilePath, {}), true);
    assertEquals(
      existsSync(tempLinkFilePath, {
        isDirectory: true,
      }),
      false,
    );
    assertEquals(
      existsSync(tempLinkFilePath, {
        isFile: true,
      }),
      true,
    );
    if (Deno.build.os !== "windows") {
      // TODO(martin-braun): include mode check for Windows tests when chmod is ported to NT
      Deno.chmodSync(tempFilePath, 0o000);
      assertEquals(
        existsSync(tempLinkFilePath, {
          isReadable: true,
        }),
        false,
      );
      // TODO(martin-braun): test unreadable link when Rust's nix::sys::stat::fchmodat has been implemented
    }
  } finally {
    if (Deno.build.os !== "windows") {
      Deno.chmodSync(tempFilePath, 0o644);
    }
    tempFile.close();
    Deno.removeSync(tempDirPath, { recursive: true });
  }
});

Deno.test("exists() returns true for an existing dir", async function () {
  const tempDirPath = await Deno.makeTempDir();
  try {
    assertEquals(await exists(tempDirPath), true);
    assertEquals(await exists(tempDirPath, {}), true);
    assertEquals(
      await exists(tempDirPath, {
        isDirectory: true,
      }),
      true,
    );
    assertEquals(
      await exists(tempDirPath, {
        isFile: true,
      }),
      false,
    );
    if (Deno.build.os !== "windows") {
      // TODO(martin-braun): include mode check for Windows tests when chmod is ported to NT
      await Deno.chmod(tempDirPath, 0o000);
      assertEquals(
        await exists(tempDirPath, {
          isReadable: true,
        }),
        false,
      );
    }
  } finally {
    if (Deno.build.os !== "windows") {
      await Deno.chmod(tempDirPath, 0o755);
    }
    await Deno.remove(tempDirPath, { recursive: true });
  }
});

Deno.test("exists() returns true for an existing dir symlink", async function () {
  const tempDirPath = await Deno.makeTempDir();
  const tempLinkDirPath = path.join(tempDirPath, "temp-link");
  try {
    await Deno.symlink(tempDirPath, tempLinkDirPath);
    assertEquals(await exists(tempLinkDirPath), true);
    assertEquals(await exists(tempLinkDirPath, {}), true);
    assertEquals(
      await exists(tempLinkDirPath, {
        isDirectory: true,
      }),
      true,
    );
    assertEquals(
      await exists(tempLinkDirPath, {
        isFile: true,
      }),
      false,
    );
    if (Deno.build.os !== "windows") {
      // TODO(martin-braun): include mode check for Windows tests when chmod is ported to NT
      await Deno.chmod(tempDirPath, 0o000);
      assertEquals(
        await exists(tempLinkDirPath, {
          isReadable: true,
        }),
        false,
      );
      // TODO(martin-braun): test unreadable link when Rust's nix::sys::stat::fchmodat has been implemented
    }
  } finally {
    if (Deno.build.os !== "windows") {
      await Deno.chmod(tempDirPath, 0o755);
    }
    await Deno.remove(tempDirPath, { recursive: true });
  }
});

Deno.test("existsSync() returns true for an existing dir", function () {
  const tempDirPath = Deno.makeTempDirSync();
  try {
    assertEquals(existsSync(tempDirPath), true);
    assertEquals(existsSync(tempDirPath, {}), true);
    assertEquals(
      existsSync(tempDirPath, {
        isDirectory: true,
      }),
      true,
    );
    assertEquals(
      existsSync(tempDirPath, {
        isFile: true,
      }),
      false,
    );
    if (Deno.build.os !== "windows") {
      // TODO(martin-braun): include mode check for Windows tests when chmod is ported to NT
      Deno.chmodSync(tempDirPath, 0o000);
      assertEquals(
        existsSync(tempDirPath, {
          isReadable: true,
        }),
        false,
      );
    }
  } finally {
    if (Deno.build.os !== "windows") {
      Deno.chmodSync(tempDirPath, 0o755);
    }
    Deno.removeSync(tempDirPath, { recursive: true });
  }
});

Deno.test("existsSync() returns true for an existing dir symlink", function () {
  const tempDirPath = Deno.makeTempDirSync();
  const tempLinkDirPath = path.join(tempDirPath, "temp-link");
  try {
    Deno.symlinkSync(tempDirPath, tempLinkDirPath);
    assertEquals(existsSync(tempLinkDirPath), true);
    assertEquals(existsSync(tempLinkDirPath, {}), true);
    assertEquals(
      existsSync(tempLinkDirPath, {
        isDirectory: true,
      }),
      true,
    );
    assertEquals(
      existsSync(tempLinkDirPath, {
        isFile: true,
      }),
      false,
    );
    if (Deno.build.os !== "windows") {
      // TODO(martin-braun): include mode check for Windows tests when chmod is ported to NT
      Deno.chmodSync(tempDirPath, 0o000);
      assertEquals(
        existsSync(tempLinkDirPath, {
          isReadable: true,
        }),
        false,
      );
      // TODO(martin-braun): test unreadable link when Rust's nix::sys::stat::fchmodat has been implemented
    }
  } finally {
    if (Deno.build.os !== "windows") {
      Deno.chmodSync(tempDirPath, 0o755);
    }
    Deno.removeSync(tempDirPath, { recursive: true });
  }
});
