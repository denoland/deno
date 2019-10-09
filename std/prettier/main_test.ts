// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { join } from "../fs/path.ts";
import { EOL } from "../fs/path/constants.ts";
import { assertEquals } from "../testing/asserts.ts";
import { test, runIfMain } from "../testing/mod.ts";
import { xrun } from "./util.ts";
import { copy, emptyDir } from "../fs/mod.ts";
const { readAll, execPath } = Deno;

const decoder = new TextDecoder();

async function run(
  args: string[]
): Promise<{ stdout: string; code: number | undefined }> {
  const p = xrun({ args, stdout: "piped" });

  const stdout = decoder.decode(await readAll(p.stdout!));
  const { code } = await p.status();

  return { stdout, code };
}

const cmd = [
  execPath(),
  "run",
  "--allow-run",
  "--allow-write",
  "--allow-read",
  "prettier/main.ts"
];
const testdata = join("prettier", "testdata");

function normalizeOutput(output: string): string {
  return output
    .replace(/\r/g, "")
    .replace(/\\/g, "/")
    .trim()
    .split("\n")
    .sort()
    .join("\n");
}

function normalizeSourceCode(source: string): string {
  return source.replace(/\r/g, "");
}

test(async function testPrettierCheckAndFormatFiles(): Promise<void> {
  const tempDir = await Deno.makeTempDir();
  await copy(testdata, tempDir, { overwrite: true });

  const files = [
    join(tempDir, "0.ts"),
    join(tempDir, "1.js"),
    join(tempDir, "2.ts")
  ];

  let p = await run([...cmd, "--check", ...files]);
  assertEquals(p.code, 1);
  assertEquals(normalizeOutput(p.stdout), "Some files are not formatted");

  p = await run([...cmd, "--write", ...files]);
  assertEquals(p.code, 0);
  assertEquals(
    normalizeOutput(p.stdout),
    normalizeOutput(`Formatting ${tempDir}/0.ts
Formatting ${tempDir}/1.js`)
  );

  p = await run([...cmd, "--check", ...files]);
  assertEquals(p.code, 0);
  assertEquals(normalizeOutput(p.stdout), "Every file is formatted");

  emptyDir(tempDir);
});

test(async function testPrettierCheckAndFormatDirs(): Promise<void> {
  const tempDir = await Deno.makeTempDir();
  await copy(testdata, tempDir, { overwrite: true });

  const dirs = [join(tempDir, "foo"), join(tempDir, "bar")];

  let p = await run([...cmd, "--check", ...dirs]);
  assertEquals(p.code, 1);
  assertEquals(normalizeOutput(p.stdout), "Some files are not formatted");

  p = await run([...cmd, "--write", ...dirs]);
  assertEquals(p.code, 0);
  assertEquals(
    normalizeOutput(p.stdout),
    normalizeOutput(`Formatting ${tempDir}/bar/0.ts
Formatting ${tempDir}/bar/1.js
Formatting ${tempDir}/foo/0.ts
Formatting ${tempDir}/foo/1.js`)
  );

  p = await run([...cmd, "--check", ...dirs]);
  assertEquals(p.code, 0);
  assertEquals(normalizeOutput(p.stdout), "Every file is formatted");

  emptyDir(tempDir);
});

test(async function testPrettierOptions(): Promise<void> {
  const tempDir = await Deno.makeTempDir();
  await copy(testdata, tempDir, { overwrite: true });

  const file0 = join(tempDir, "opts", "0.ts");
  const file1 = join(tempDir, "opts", "1.ts");
  const file2 = join(tempDir, "opts", "2.ts");
  const file3 = join(tempDir, "opts", "3.md");

  const getSourceCode = async (f: string): Promise<string> =>
    decoder.decode(await Deno.readFile(f));

  await run([...cmd, "--no-semi", "--write", file0]);
  assertEquals(
    normalizeSourceCode(await getSourceCode(file0)),
    `console.log(0)
console.log([function foo() {}, function baz() {}, a => {}])
`
  );

  await run([
    ...cmd,
    "--print-width",
    "30",
    "--tab-width",
    "4",
    "--write",
    file0
  ]);
  assertEquals(
    normalizeSourceCode(await getSourceCode(file0)),
    `console.log(0);
console.log([
    function foo() {},
    function baz() {},
    a => {}
]);
`
  );

  await run([...cmd, "--print-width", "30", "--use-tabs", "--write", file0]);
  assertEquals(
    normalizeSourceCode(await getSourceCode(file0)),
    `console.log(0);
console.log([
	function foo() {},
	function baz() {},
	a => {}
]);
`
  );

  await run([...cmd, "--single-quote", "--write", file1]);
  assertEquals(
    normalizeSourceCode(await getSourceCode(file1)),
    `console.log('1');
`
  );

  await run([
    ...cmd,
    "--print-width",
    "30",
    "--trailing-comma",
    "all",
    "--write",
    file0
  ]);
  assertEquals(
    normalizeSourceCode(await getSourceCode(file0)),
    `console.log(0);
console.log([
  function foo() {},
  function baz() {},
  a => {},
]);
`
  );

  await run([...cmd, "--no-bracket-spacing", "--write", file2]);
  assertEquals(
    normalizeSourceCode(await getSourceCode(file2)),
    `console.log({a: 1});
`
  );

  await run([...cmd, "--arrow-parens", "always", "--write", file0]);
  assertEquals(
    normalizeSourceCode(await getSourceCode(file0)),
    `console.log(0);
console.log([function foo() {}, function baz() {}, (a) => {}]);
`
  );

  await run([...cmd, "--prose-wrap", "always", "--write", file3]);
  assertEquals(
    normalizeSourceCode(await getSourceCode(file3)),
    "Lorem ipsum dolor sit amet, consectetur adipiscing elit, " +
      "sed do eiusmod tempor" +
      "\nincididunt ut labore et dolore magna aliqua.\n"
  );

  await run([...cmd, "--end-of-line", "crlf", "--write", file2]);
  assertEquals(await getSourceCode(file2), "console.log({ a: 1 });\r\n");

  emptyDir(tempDir);
});

test(async function testPrettierPrintToStdout(): Promise<void> {
  const tempDir = await Deno.makeTempDir();
  await copy(testdata, tempDir, { overwrite: true });

  const file0 = join(tempDir, "0.ts");
  const file1 = join(tempDir, "formatted.ts");

  const getSourceCode = async (f: string): Promise<string> =>
    decoder.decode(await Deno.readFile(f));

  const { stdout } = await run([...cmd, file0]);
  // The source file will not change without `--write` flags.
  assertEquals(await getSourceCode(file0), "console.log (0)" + EOL);
  // The output should be formatted code.
  assertEquals(stdout, "console.log(0);" + EOL);

  const { stdout: formattedCode } = await run([...cmd, file1]);
  // The source file will not change without `--write` flags.
  assertEquals(await getSourceCode(file1), "console.log(0);" + EOL);
  // The output will be formatted code even it is the same as the source file's
  // content.
  assertEquals(formattedCode, "console.log(0);" + EOL);

  emptyDir(tempDir);
});

test(async function testPrettierReadFromStdin(): Promise<void> {
  interface TestCase {
    stdin: string;
    stdout: string;
    stderr: string;
    code: number;
    success: boolean;
    parser?: string;
  }

  async function readFromStdinAssertion(
    stdin: string,
    expectedStdout: string,
    expectedStderr: string,
    expectedCode: number,
    expectedSuccess: boolean,
    parser?: string
  ): Promise<void> {
    const inputCode = stdin;
    const p1 = Deno.run({
      args: [execPath(), "./prettier/testdata/echox.ts", `${inputCode}`],
      stdout: "piped"
    });

    const p2 = Deno.run({
      args: [
        execPath(),
        "run",
        "./prettier/main.ts",
        "--stdin",
        ...(parser ? ["--stdin-parser", parser] : [])
      ],
      stdin: "piped",
      stdout: "piped",
      stderr: "piped"
    });

    const n = await Deno.copy(p2.stdin!, p1.stdout!);
    assertEquals(n, new TextEncoder().encode(stdin).length);

    const status1 = await p1.status();
    assertEquals(status1.code, 0);
    assertEquals(status1.success, true);
    p2.stdin!.close();
    const status2 = await p2.status();
    assertEquals(status2.code, expectedCode);
    assertEquals(status2.success, expectedSuccess);
    const decoder = new TextDecoder("utf-8");
    assertEquals(
      decoder.decode(await Deno.readAll(p2.stdout!)),
      expectedStdout
    );
    assertEquals(
      decoder.decode(await Deno.readAll(p2.stderr!)).split(EOL)[0],
      expectedStderr
    );
    p2.close();
    p1.close();
  }

  const testCases: TestCase[] = [
    {
      stdin: `console.log("abc"  )`,
      stdout: `console.log("abc");\n`,
      stderr: ``,
      code: 0,
      success: true
    },
    {
      stdin: `console.log("abc"  )`,
      stdout: `console.log("abc");\n`,
      stderr: ``,
      code: 0,
      success: true,
      parser: "babel"
    },
    {
      stdin: `{\"a\":\"b\"}`,
      stdout: `{ "a": "b" }\n`,
      stderr: ``,
      code: 0,
      success: true,
      parser: "json"
    },
    {
      stdin: `##  test`,
      stdout: `## test\n`,
      stderr: ``,
      code: 0,
      success: true,
      parser: "markdown"
    },
    {
      stdin: `invalid typescript code##!!@@`,
      stdout: ``,
      stderr: `SyntaxError: ';' expected. (1:9)`,
      code: 1,
      success: false
    },
    {
      stdin: `console.log("foo");`,
      stdout: ``,
      stderr:
        'Error: Couldn\'t resolve parser "invalid_parser". ' +
        "Parsers must be explicitly added to the standalone bundle.",
      code: 1,
      success: false,
      parser: "invalid_parser"
    }
  ];

  for (const t of testCases) {
    await readFromStdinAssertion(
      t.stdin,
      t.stdout,
      t.stderr,
      t.code,
      t.success,
      t.parser
    );
  }
});

runIfMain(import.meta);
