// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import * as fs from "../fs.ts";
import { assertRejects, assertThrows } from "../../testing/asserts.ts";

Deno.test(
  "[node/fs.access] Uses the owner permission when the user is the owner",
  { ignore: Deno.build.os === "windows" },
  async () => {
    const file = await Deno.makeTempFile();
    try {
      Deno.chmod(file, 0o600);
      await fs.promises.access(file, fs.constants.R_OK);
      await fs.promises.access(file, fs.constants.W_OK);
      await assertRejects(async () => {
        await fs.promises.access(file, fs.constants.X_OK);
      });
    } finally {
      await Deno.remove(file);
    }
  },
);

Deno.test(
  "[node/fs.access] doesn't reject on windows",
  { ignore: Deno.build.os !== "windows" },
  async () => {
    const file = await Deno.makeTempFile();
    try {
      await fs.promises.access(file, fs.constants.R_OK);
      await fs.promises.access(file, fs.constants.W_OK);
    } finally {
      await Deno.remove(file);
    }
  },
);

Deno.test(
  "[node/fs.accessSync] Uses the owner permission when the user is the owner",
  { ignore: Deno.build.os === "windows" },
  () => {
    const file = Deno.makeTempFileSync();
    try {
      Deno.chmod(file, 0o600);
      fs.accessSync(file, fs.constants.R_OK);
      fs.accessSync(file, fs.constants.W_OK);
      assertThrows(() => {
        fs.accessSync(file, fs.constants.X_OK);
      });
    } finally {
      Deno.removeSync(file);
    }
  },
);

Deno.test(
  "[node/fs.accessSync] doesn't throw on windows",
  { ignore: Deno.build.os !== "windows" },
  () => {
    const file = Deno.makeTempFileSync();
    try {
      fs.accessSync(file, fs.constants.R_OK);
      fs.accessSync(file, fs.constants.W_OK);
    } finally {
      Deno.removeSync(file);
    }
  },
);
