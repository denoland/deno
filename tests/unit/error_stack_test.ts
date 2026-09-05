// Copyright 2018-2026 the Deno authors. MIT license.
import {
  assertEquals,
  assertFalse,
  assertMatch,
  assertStringIncludes,
  pathToAbsoluteFileUrl,
} from "./test_util.ts";

async function runDeno(args: string[]) {
  const command = new Deno.Command(Deno.execPath(), {
    args,
    clearEnv: true,
    stdout: "piped",
    stderr: "piped",
  });
  const { code, stdout, stderr } = await command.output();
  const decoder = new TextDecoder();
  return {
    code,
    stdout: decoder.decode(stdout),
    stderr: decoder.decode(stderr),
  };
}

Deno.test(function errorStackMessageLine() {
  const e1 = new Error();
  e1.name = "Foo";
  e1.message = "bar";
  assertMatch(e1.stack!, /^Foo: bar\n/);

  const e2 = new Error();
  e2.name = "";
  e2.message = "bar";
  assertMatch(e2.stack!, /^bar\n/);

  const e3 = new Error();
  e3.name = "Foo";
  e3.message = "";
  assertMatch(e3.stack!, /^Foo\n/);

  const e4 = new Error();
  e4.name = "";
  e4.message = "";
  assertMatch(e4.stack!, /^\n/);

  const e5 = new Error();
  // deno-lint-ignore ban-ts-comment
  // @ts-expect-error
  e5.name = undefined;
  // deno-lint-ignore ban-ts-comment
  // @ts-expect-error
  e5.message = undefined;
  assertMatch(e5.stack!, /^Error\n/);

  const e6 = new Error();
  // deno-lint-ignore ban-ts-comment
  // @ts-expect-error
  e6.name = null;
  // deno-lint-ignore ban-ts-comment
  // @ts-expect-error
  e6.message = null;
  assertMatch(e6.stack!, /^null: null\n/);
});

Deno.test(function captureStackTrace() {
  function foo() {
    const error = new Error();
    const stack1 = error.stack!;
    Error.captureStackTrace(error, foo);
    const stack2 = error.stack!;
    // stack2 should be stack1 without the first frame.
    assertEquals(stack2, stack1.replace(/(?<=^[^\n]*\n)[^\n]*\n/, ""));
  }
  foo();
});

Deno.test({
  name: "diagnostic source file reads use runtime permissions",
  permissions: {
    run: [Deno.execPath()],
    write: true,
  },
  async fn() {
    const tempDir = Deno.makeTempDirSync();
    try {
      const sourcePath = `${tempDir}/original.ts`;
      const sourceLine = 'const value = "DIAGNOSTIC_SOURCE_LINE";';
      Deno.writeTextFileSync(sourcePath, `${sourceLine}\n`);
      const sourceMap = {
        version: 3,
        sources: [pathToAbsoluteFileUrl(sourcePath).href],
        names: [],
        mappings: "AAAA",
      };

      const inlineScriptPath = `${tempDir}/inline.js`;
      const inlineSourceMap = btoa(JSON.stringify(sourceMap));
      Deno.writeTextFileSync(
        inlineScriptPath,
        `const error = new Error("inline"); const result = Deno[Deno.internal].core.destructureError(error); console.log(result.sourceLine ?? "NO_SOURCE_LINE");\n//# sourceMappingURL=data:application/json;base64,${inlineSourceMap}\n`,
      );
      const inlineResult = await runDeno([
        "run",
        "--quiet",
        inlineScriptPath,
      ]);
      assertEquals(inlineResult.code, 0);
      assertStringIncludes(inlineResult.stdout, "NO_SOURCE_LINE");
      assertFalse(
        inlineResult.stdout.includes("DIAGNOSTIC_SOURCE_LINE"),
        inlineResult.stdout,
      );

      const externalMapPath = `${tempDir}/external.js.map`;
      const externalScriptPath = `${tempDir}/external.js`;
      Deno.writeTextFileSync(externalMapPath, JSON.stringify(sourceMap));
      Deno.writeTextFileSync(
        externalScriptPath,
        `throw new Error("external");\n//# sourceMappingURL=${
          pathToAbsoluteFileUrl(externalMapPath).href
        }\n`,
      );

      const externalDenied = await runDeno([
        "run",
        "--quiet",
        `--allow-read=${sourcePath}`,
        externalScriptPath,
      ]);
      assertEquals(externalDenied.code, 1);
      assertStringIncludes(externalDenied.stderr, "Error: external");
      assertFalse(
        externalDenied.stderr.includes("DIAGNOSTIC_SOURCE_LINE"),
        externalDenied.stderr,
      );
      assertFalse(
        externalDenied.stderr.includes("Deno requests read access"),
        externalDenied.stderr,
      );

      const externalAllowed = await runDeno([
        "run",
        "--quiet",
        `--allow-read=${tempDir}`,
        externalScriptPath,
      ]);
      assertEquals(externalAllowed.code, 1);
      assertStringIncludes(
        externalAllowed.stderr,
        "DIAGNOSTIC_SOURCE_LINE",
      );

      const externalPartiallyGranted = await runDeno([
        "run",
        "--quiet",
        `--allow-read=${externalMapPath},${sourcePath}`,
        `--deny-read=${externalMapPath}/nested`,
        externalScriptPath,
      ]);
      assertEquals(externalPartiallyGranted.code, 1);
      assertStringIncludes(
        externalPartiallyGranted.stderr,
        "DIAGNOSTIC_SOURCE_LINE",
      );

      const vmScriptPath = `${tempDir}/vm.js`;
      Deno.writeTextFileSync(
        vmScriptPath,
        `import { runInThisContext } from "node:vm";
runInThisContext('throw new Error("vm diagnostic");', {
  filename: ${JSON.stringify(pathToAbsoluteFileUrl(sourcePath).href)},
});
`,
      );
      const vmResult = await runDeno(["run", "--quiet", vmScriptPath]);
      assertEquals(vmResult.code, 1);
      assertStringIncludes(vmResult.stderr, "Error: vm diagnostic");
      assertFalse(
        vmResult.stderr.includes("DIAGNOSTIC_SOURCE_LINE"),
        vmResult.stderr,
      );
      assertFalse(
        vmResult.stderr.includes("Deno requests read access"),
        vmResult.stderr,
      );
    } finally {
      Deno.removeSync(tempDir, { recursive: true });
    }
  },
});

Deno.test({
  name: "prepared graph sources remain available to diagnostics",
  permissions: {
    run: [Deno.execPath()],
    write: true,
  },
  async fn() {
    const tempDir = Deno.makeTempDirSync();
    try {
      const staticDependency = `${tempDir}/static_dependency.ts`;
      Deno.writeTextFileSync(
        staticDependency,
        `const value: string = "static";
throw new Error(value); // STATIC_GRAPH_SOURCE
`,
      );
      const staticMain = `${tempDir}/static_main.ts`;
      Deno.writeTextFileSync(
        staticMain,
        'import "./static_dependency.ts";\n',
      );

      const staticResult = await runDeno([
        "run",
        "--quiet",
        staticMain,
      ]);
      assertEquals(staticResult.code, 1);
      assertStringIncludes(staticResult.stderr, "STATIC_GRAPH_SOURCE");

      const dynamicDependency = `${tempDir}/dynamic_dependency.ts`;
      Deno.writeTextFileSync(
        dynamicDependency,
        `const value: string = "dynamic";
throw new Error(value); // DYNAMIC_GRAPH_SOURCE
`,
      );
      const dynamicMain = `${tempDir}/dynamic_main.ts`;
      Deno.writeTextFileSync(
        dynamicMain,
        'await import("./dynamic_dependency.ts");\n',
      );

      const dynamicResult = await runDeno([
        "run",
        "--quiet",
        dynamicMain,
      ]);
      assertEquals(dynamicResult.code, 1);
      assertStringIncludes(dynamicResult.stderr, "DYNAMIC_GRAPH_SOURCE");
    } finally {
      Deno.removeSync(tempDir, { recursive: true });
    }
  },
});
