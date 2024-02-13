// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { stripColor } from "../fmt/colors.ts";
import { dirname, fromFileUrl, join, toFileUrl } from "../path/mod.ts";
import {
  assert,
  assertInstanceOf,
  AssertionError,
  assertRejects,
  fail,
} from "../assert/mod.ts";
import { assertSnapshot, createAssertSnapshot, serialize } from "./snapshot.ts";

const SNAPSHOT_MODULE_URL = toFileUrl(join(
  dirname(fromFileUrl(import.meta.url)),
  "snapshot.ts",
));

function formatTestOutput(string: string) {
  // Strip colors and obfuscate any timings
  return stripColor(string).replace(/([0-9])+m?s/g, "--ms").replace(
    /(?<=running ([0-9])+ test(s)? from )(.*)(?=test.ts)/g,
    "<tempDir>/",
  );
}

function formatTestError(string: string) {
  // Strip colors and remove "Check file:///workspaces/deno_std/testing/.tmp/test.ts"
  // as this is always output to stderr
  return stripColor(string).replace(/^Check file:\/\/(.+)\n/gm, "");
}

function testFnWithTempDir(
  fn: (t: Deno.TestContext, tempDir: string) => Promise<void>,
) {
  return async (t: Deno.TestContext) => {
    const tempDir = await Deno.makeTempDir();
    try {
      await fn(t, tempDir);
    } finally {
      await Deno.remove(tempDir, { recursive: true });
    }
  };
}

function testFnWithDifferentTempDir(
  fn: (
    t: Deno.TestContext,
    tempDir1: string,
    tempDir2: string,
  ) => Promise<void>,
) {
  return async (t: Deno.TestContext) => {
    const tempDir1 = await Deno.makeTempDir();
    const tempDir2 = await Deno.makeTempDir();
    try {
      await fn(t, tempDir1, tempDir2);
    } finally {
      await Deno.remove(tempDir1, { recursive: true });
      await Deno.remove(tempDir2, { recursive: true });
    }
  };
}

class TestClass {
  a = 1;
  b = 2;
  init() {
    this.b = 3;
  }
  get getA() {
    return this.a;
  }
  func() {}
}

const map = new Map();
map.set("Hello", "World!");
map.set(() => "Hello", "World!");
map.set(1, 2);

Deno.test("Snapshot Test", async (t) => {
  await assertSnapshot(t, { a: 1, b: 2 });
  await assertSnapshot(t, new TestClass());
  await assertSnapshot(t, map);
  await assertSnapshot(t, new Set([1, 2, 3]));
  await assertSnapshot(t, { fn() {} });
  await assertSnapshot(t, function fn() {});
  await assertSnapshot(t, [1, 2, 3]);
  await assertSnapshot(t, "hello world");
});

Deno.test("Snapshot Test - step", async (t) => {
  await assertSnapshot(t, { a: 1, b: 2 });
  await t.step("Nested", async (t) => {
    await assertSnapshot(t, new TestClass());
    await assertSnapshot(t, map);
    await t.step("Nested Nested", async (t) => {
      await assertSnapshot(t, new Set([1, 2, 3]));
      await assertSnapshot(t, { fn() {} });
      await assertSnapshot(t, function fn() {});
    });
    await assertSnapshot(t, [1, 2, 3]);
  });
  await assertSnapshot(t, "hello world");
});

Deno.test("Snapshot Test - Adverse String \\ ` ${}", async (t) => {
  await assertSnapshot(t, "\\ ` ${}");
});

Deno.test("Snapshot Test - Default serializer", async (t) => {
  await assertSnapshot(t, "a\nb\tc");
});

Deno.test("Snapshot Test - Multi-Line Strings", async (t) => {
  await t.step("string", async (t) => {
    await assertSnapshot(
      t,
      `
<html>
  <head>
    <title>Snapshot Test - Multi-Line Strings</title>
  </head>
  <body>
    <h1>
      Snapshot Test - Multi-Line Strings
    </h2>
    <p>
      This is a snapshot of a multi-line string.
    </p>
  </body>
</html>`,
    );
  });

  await t.step("string in array", async (t) => {
    await assertSnapshot(t, [
      `
<h1>
  Header
</h1>`,
      `
<p>
  Content
</p>`,
    ]);
  });

  await t.step("string in object", async (t) => {
    await assertSnapshot(t, {
      str: `
        Line #1
        Line #2
        Line #3`,
    });
  });
});

Deno.test(
  "Snapshot Test - Failed Assertion",
  testFnWithTempDir(async (t, tempDir) => {
    let count = 0;
    async function testFailedAssertion<T>(
      snapshot: T,
      actual: T,
    ): Promise<AssertionError> {
      const snapshotFilePath = join(tempDir, `snapshot_file_${++count}.snap`);
      await Deno.writeTextFile(
        snapshotFilePath,
        `export const snapshot = {};

snapshot[\`name 1\`] = \`
${serialize(snapshot)}
\`;
`,
      );

      try {
        await assertSnapshot(t, actual, {
          path: snapshotFilePath,
          mode: "assert",
          name: "name",
        });
        fail("Snapshot assertion passed when it was expected to fail");
      } catch (error) {
        assertInstanceOf(error, AssertionError);
        return error as AssertionError;
      }
    }

    await t.step("Object", async (t) => {
      const error = await testFailedAssertion([1, 2, 3], [1, 2]);
      await assertSnapshot(t, stripColor(error.message));
    });

    await t.step("String", async (t) => {
      const error = await testFailedAssertion("Hello World!", "Hello!");
      await assertSnapshot(t, stripColor(error.message));
    });
  }),
);

Deno.test("Snapshot Test - Options", async (t) => {
  const VALUE = [1, 2, 3];

  await t.step("dir", async (t) => {
    await t.step("relative", async (t) => {
      await assertSnapshot(t, VALUE, {
        dir: "__snapshots__/options_tests/",
      });
    });

    await t.step("absolute", async (t) => {
      await assertSnapshot(t, VALUE, {
        dir: join(Deno.cwd(), "testing/__snapshots__/options_tests/"),
      });
    });
  });

  await t.step("path", async (t) => {
    await t.step("relative", async (t) => {
      await assertSnapshot(t, VALUE, {
        path: "__snapshots__/options_tests/custom_path.snap",
      });
    });

    await t.step("absolute", async (t) => {
      await assertSnapshot(t, VALUE, {
        path: join(
          Deno.cwd(),
          "testing/__snapshots__/options_tests/custom_path.snap",
        ),
      });
    });
  });

  await t.step("name", async (t) => {
    await assertSnapshot(t, VALUE, {
      name: "custom name",
    });

    await assertSnapshot(t, VALUE, {
      name: "custom name",
    });
  });

  await t.step("serializer", async (t) => {
    await assertSnapshot<Array<number>>(t, VALUE, {
      serializer: (actual) => {
        return `Array Length: ${actual.length}\n\n${serialize(actual)}`;
      },
    });
  });

  await t.step("msg", async (t) => {
    await t.step("missing snapshot", async (t) => {
      try {
        await assertSnapshot<Array<number>>(t, VALUE, {
          msg: "[CUSTOM ERROR MESSAGE - MISSING SNAPSHOT]",
          mode: "assert",
          name: "MISSING SNAPSHOT",
        });
        fail("Snapshot should not exist");
      } catch (error) {
        assertInstanceOf(error, AssertionError);
        await assertSnapshot(t, error.message);
      }
    });

    await t.step("missing snapshot file", async (t) => {
      try {
        await assertSnapshot<Array<number>>(t, VALUE, {
          msg: "[CUSTOM ERROR MESSAGE - MISSING SNAPSHOT]",
          mode: "assert",
          path: "MISSING_SNAPSHOT_FILE.snap",
        });
        fail("Snapshot file should not exist");
      } catch (error) {
        assertInstanceOf(error, AssertionError);
        await assertSnapshot(t, error.message);
      }
    });
  });

  await t.step(
    "mode",
    testFnWithTempDir(async (t, tempDir) => {
      const snapshotFilePath = join(tempDir, "snapshot.snap");
      const snapshotName = "snapshot";

      async function runTest(test: string) {
        const tempTestFileName = "test.ts";
        const tempTestFilePath = join(tempDir, tempTestFileName);
        await Deno.writeTextFile(tempTestFilePath, test);

        const process = new Deno.Command(Deno.execPath(), {
          args: [
            "test",
            "--allow-all",
            tempTestFilePath,
            "--",
            "-u",
          ],
          stdout: "piped",
          stderr: "piped",
        });
        const { stdout, stderr } = await process.output();

        return {
          output: new TextDecoder().decode(stdout),
          error: new TextDecoder().decode(stderr),
        };
      }

      const result = await runTest(`
        import { assertSnapshot } from "${SNAPSHOT_MODULE_URL}";

        Deno.test("${snapshotName}", async (t) => {
          await assertSnapshot(t, [1, 2, 3], {
            path: "${snapshotFilePath.replace(/\\/g, "\\\\")}",
            mode: "update",
          });
        });
      `);

      const { snapshot } = await import(toFileUrl(snapshotFilePath).toString());

      await assertSnapshot(t, snapshot[`${snapshotName} 1`]);
      await assertSnapshot(t, formatTestOutput(result.output));
      assert(!formatTestError(result.error), "unexpected output to stderr");
    }),
  );
});

Deno.test(
  "Snapshot Test - Update",
  testFnWithTempDir(async (t, tempDir) => {
    const tempTestFileName = "test.ts";
    const tempTestFilePath = join(tempDir, tempTestFileName);
    const tempSnapshotFilePath = join(
      tempDir,
      "__snapshots__",
      `${tempTestFileName}.snap`,
    );

    async function runTestWithUpdateFlag(test: string) {
      await Deno.writeTextFile(tempTestFilePath, test);

      const command = new Deno.Command(Deno.execPath(), {
        args: ["test", "--allow-all", tempTestFilePath, "--", "-u"],
      });
      const { stdout, stderr } = await command.output();

      return {
        output: new TextDecoder().decode(stdout),
        error: new TextDecoder().decode(stderr),
        snapshots: await Deno.readTextFile(tempSnapshotFilePath),
      };
    }

    function assertNoError(error: string) {
      if (formatTestError(error)) {
        throw new AssertionError(`Unexpected Error:\n\n${error}\n`);
      }
    }

    /**
     * New snapshot
     */
    const result1 = await runTestWithUpdateFlag(
      `
      import { assertSnapshot } from "${SNAPSHOT_MODULE_URL}";

      Deno.test("Snapshot Test - Update", async (t) => {
        await assertSnapshot(t, [
          1,
          2,
        ]);
      });`,
    );

    assertNoError(result1.error);
    await assertSnapshot(t, formatTestOutput(result1.output), {
      name: "Snapshot Test - Update - New snapshot",
    });
    await assertSnapshot(t, result1.snapshots, {
      name: "Snapshot Test - Update - New snapshot",
    });

    /**
     * Existing snapshot - no changes
     */
    const result2 = await runTestWithUpdateFlag(
      `
      import { assertSnapshot } from "${SNAPSHOT_MODULE_URL}";

      Deno.test("Snapshot Test - Update", async (t) => {
        await assertSnapshot(t, [
          1,
          2,
        ]);
      });`,
    );

    assertNoError(result2.error);
    await assertSnapshot(t, formatTestOutput(result2.output), {
      name: "Snapshot Test - Update - Existing snapshot - no changes",
    });
    await assertSnapshot(t, result2.snapshots, {
      name: "Snapshot Test - Update - Existing snapshot - no changes",
    });

    /**
     * Existing snapshot - updates
     */
    const result3 = await runTestWithUpdateFlag(`
      import { assertSnapshot } from "${SNAPSHOT_MODULE_URL}";

      Deno.test("Snapshot Test - Update", async (t) => {
        await assertSnapshot(t, [
          1,
          2,
          3,
          5,
        ]);
      });
    `);

    assertNoError(result3.error);
    await assertSnapshot(t, formatTestOutput(result3.output), {
      name: "Snapshot Test - Update - Existing snapshot - updates",
    });
    await assertSnapshot(t, result3.snapshots, {
      name: "Snapshot Test - Update - Existing snapshot - updates",
    });

    /**
     * Existing snapshots - reverse order 1
     */
    const result4 = await runTestWithUpdateFlag(`
      import { assertSnapshot } from "${SNAPSHOT_MODULE_URL}";

      Deno.test("Snapshot Test - First", async (t) => {
        await assertSnapshot(t, "FIRST");
      });

      Deno.test("Snapshot Test - Second", async (t) => {
        await assertSnapshot(t, "SECOND");
      });
   `);

    assertNoError(result4.error);
    await assertSnapshot(t, formatTestOutput(result4.output), {
      name: "Snapshot Test - Update - Existing snapshots - reverse order 1",
    });
    await assertSnapshot(t, result4.snapshots, {
      name: "Snapshot Test - Update - Existing snapshots - reverse order 1",
    });

    /**
     * Existing snapshots - reverse order 2
     */
    const result5 = await runTestWithUpdateFlag(`
      import { assertSnapshot } from "${SNAPSHOT_MODULE_URL}";

      Deno.test("Snapshot Test - Second", async (t) => {
        await assertSnapshot(t, "SECOND");
      });

      Deno.test("Snapshot Test - First", async (t) => {
        await assertSnapshot(t, "FIRST");
      });
   `);

    assertNoError(result5.error);
    await assertSnapshot(t, formatTestOutput(result5.output), {
      name: "Snapshot Test - Update - Existing snapshots - reverse order 2",
    });
    await assertSnapshot(t, result5.snapshots, {
      name: "Snapshot Test - Update - Existing snapshots - reverse order 2",
    });
  }),
);

Deno.test(
  "Snapshot Test - Remove",
  testFnWithTempDir(async (t, tempDir) => {
    const tempTestFileName = "test.ts";
    const tempTestFilePath = join(tempDir, tempTestFileName);

    async function runTestWithUpdateFlag(test: string) {
      await Deno.writeTextFile(tempTestFilePath, test);

      const command = new Deno.Command(Deno.execPath(), {
        args: ["test", "--allow-all", tempTestFilePath, "--", "-u"],
      });
      const { stdout, stderr } = await command.output();

      return {
        output: new TextDecoder().decode(stdout),
        error: new TextDecoder().decode(stderr),
      };
    }

    function assertNoError(error: string) {
      if (formatTestError(error)) {
        throw new AssertionError(`Unexpected Error:\n\n${error}\n`);
      }
    }

    /**
     * New snapshot
     */
    const result1 = await runTestWithUpdateFlag(
      `
      import { assertSnapshot } from "${SNAPSHOT_MODULE_URL}";

      Deno.test("Snapshot Test - Remove - First", async (t) => {
        await assertSnapshot(t, { a: 1, b: 2 });
      });

      Deno.test("Snapshot Test - Remove - Second", async (t) => {
        await assertSnapshot(t, { c: 3, d: 4 });
      });

      Deno.test("Snapshot Test - Remove - Third", async (t) => {
        await assertSnapshot(t, { e: 5, f: 6 });
      });

      Deno.test("Snapshot Test - Remove - Fourth", async (t) => {
        await assertSnapshot(t, { g: 7, h: 8 });
      });

      Deno.test("Snapshot Test - Remove - Fifth", async (t) => {
        await assertSnapshot(t, { i: 9, j: 10 });
      });
      `,
    );

    assertNoError(result1.error);
    await assertSnapshot(t, formatTestOutput(result1.output), {
      name: "Snapshot Test - Remove - New snapshot",
    });

    /**
     * Existing snapshot - removes one
     */
    const result2 = await runTestWithUpdateFlag(
      `
      import { assertSnapshot } from "${SNAPSHOT_MODULE_URL}";

      Deno.test("Snapshot Test - Remove - First", async (t) => {
        await assertSnapshot(t, { a: 1, b: 2 });
      });

      Deno.test("Snapshot Test - Remove - Second", async (t) => {
        await assertSnapshot(t, { c: 3, d: 4 });
      });

      Deno.test("Snapshot Test - Remove - Fourth", async (t) => {
        await assertSnapshot(t, { g: 7, h: 8 });
      });

      Deno.test("Snapshot Test - Remove - Fifth", async (t) => {
        await assertSnapshot(t, { i: 9, j: 10 });
      });
      `,
    );

    assertNoError(result2.error);
    await assertSnapshot(t, formatTestOutput(result2.output), {
      name: "Snapshot Test - Remove - Existing snapshot - removed one",
    });

    /**
     * Existing snapshot - removes several
     */
    const result3 = await runTestWithUpdateFlag(
      `
      import { assertSnapshot } from "${SNAPSHOT_MODULE_URL}";

      Deno.test("Snapshot Test - Remove - First", async (t) => {
        await assertSnapshot(t, { a: 1, b: 2 });
      });
      `,
    );

    assertNoError(result3.error);
    await assertSnapshot(t, formatTestOutput(result3.output), {
      name: "Snapshot Test - Remove - Existing snapshot - removed several",
    });
  }),
);

Deno.test(
  "Snapshot Test - Different Dir",
  testFnWithDifferentTempDir(async (t, tempDir1, tempDir2) => {
    const tempTestFileName = "test.ts";
    const tempTestFilePath1 = join(tempDir1, tempTestFileName);
    const tempTestFilePath2 = join(tempDir2, tempTestFileName);

    async function runTestWithUpdateFlag(test1: string, test2: string) {
      await Deno.writeTextFile(tempTestFilePath1, test1);
      await Deno.writeTextFile(tempTestFilePath2, test2);

      const command = new Deno.Command(Deno.execPath(), {
        args: [
          "test",
          "--allow-all",
          tempTestFilePath1,
          tempTestFilePath2,
          "--",
          "-u",
        ],
      });
      const { stdout, stderr } = await command.output();

      return {
        output: new TextDecoder().decode(stdout),
        error: new TextDecoder().decode(stderr),
      };
    }

    function assertNoError(error: string) {
      if (formatTestError(error)) {
        throw new AssertionError(`Unexpected Error:\n\n${error}\n`);
      }
    }

    /**
     * New snapshot
     */
    const result1 = await runTestWithUpdateFlag(
      `
      import { assertSnapshot } from "${SNAPSHOT_MODULE_URL}";

      Deno.test("Snapshot Test - First", async (t) => {
        await assertSnapshot(t, [
          1,
          2,
        ]);
      });
      Deno.test("Snapshot Test - Second", async (t) => {
        await assertSnapshot(t, [
          3,
          4,
        ]);
      });`,
      `
      import { assertSnapshot } from "${SNAPSHOT_MODULE_URL}";

      Deno.test("Snapshot Test - First", async (t) => {
        await assertSnapshot(t, [
          1,
          2,
        ]);
      });
      Deno.test("Snapshot Test - Second", async (t) => {
        await assertSnapshot(t, [
          3,
          4,
        ]);
      });`,
    );

    assertNoError(result1.error);
    await assertSnapshot(t, formatTestOutput(result1.output), {
      name: "Snapshot Test - Different Dir - New snapshot",
    });

    /**
     * Existing snapshot - updates
     */
    const result2 = await runTestWithUpdateFlag(
      `
      import { assertSnapshot } from "${SNAPSHOT_MODULE_URL}";

      Deno.test("Snapshot Test - First", async (t) => {
        await assertSnapshot(t, [
          1,
          2,
        ]);
      });
      Deno.test("Snapshot Test - Second", async (t) => {
        await assertSnapshot(t, [
          3,
          5,
        ]);
      });`,
      `
      import { assertSnapshot } from "${SNAPSHOT_MODULE_URL}";

      Deno.test("Snapshot Test - First", async (t) => {
        await assertSnapshot(t, [
          6,
          7,
        ]);
      });
      Deno.test("Snapshot Test - Second", async (t) => {
        await assertSnapshot(t, [
          8,
          9,
        ]);
      });`,
    );
    assertNoError(result2.error);
    await assertSnapshot(t, formatTestOutput(result2.output), {
      name: "Snapshot Test - Different Dir - Existing snapshot - update",
    });
  }),
);

// Regression test for https://github.com/denoland/deno_std/issues/2140
// Long strings should not be truncated with ellipsis
Deno.test("Snapshot Test - Regression #2140", async (t) => {
  await assertSnapshot(t, {
    title: "Testing a page",
    content: `
      <h1>Testing a page</h1>
      <p>This is a test</p>
      <ul>
        <li>1</li>
        <li>2</li>
        <li>3</li>
        <li>4</li>
      </ul>
      `,
  });
});

// Regression test for https://github.com/denoland/deno_std/issues/2144
// Empty arrays should be compacted
Deno.test("Snapshot Test - Regression #2144", async (t) => {
  const config = {
    fmt: {
      files: {
        exclude: [],
        include: [],
      },
      options: {},
    },
  };
  await assertSnapshot(t, config);
});

Deno.test("Snapshot Test - Empty #2245", async (t) => {
  await assertSnapshot(t, "", { serializer: (x) => x });
});

Deno.test("SnapshotTest - createAssertSnapshot", async (t) => {
  const assertMonochromeSnapshot = createAssertSnapshot<string>({
    serializer: stripColor,
  });

  await t.step("No Options", async (t) => {
    await assertMonochromeSnapshot(
      t,
      "\x1b[32mThis green text has had it's colours stripped\x1b[39m",
    );
  });

  await t.step("Options Object", async (t) => {
    await assertMonochromeSnapshot(
      t,
      "\x1b[32mThis green text has had it's colours stripped\x1b[39m",
      {
        name:
          "SnapshotTest - createAssertSnapshot - Options Object - Custom Name",
      },
    );
  });

  await t.step("Message", async (t) => {
    const assertMissingSnapshot = createAssertSnapshot<string>({
      mode: "assert",
      name: "[MISSING SNAPSHOT]",
    });

    const err = await assertRejects(async () => {
      await assertMissingSnapshot(
        t,
        null,
        "This snapshot has failed as expected",
      );
    }, AssertionError);

    await assertSnapshot(t, err.message);
  });

  await t.step("Composite", async (t) => {
    const assertMonochromeSnapshotComposite = createAssertSnapshot<string>({
      name: "SnapshotTest - createAssertSnapshot - Composite - Custom Name",
    }, assertMonochromeSnapshot);

    await assertMonochromeSnapshotComposite(
      t,
      "\x1b[32mThis green text has had it's colours stripped\x1b[39m",
    );
  });
});
