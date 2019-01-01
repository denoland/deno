import { cwd, exit, RunOptions } from "deno";
import {
  findFiles,
  ProcessOptions,
  resolveProcess,
  ProcessResult
} from "./third_party.ts";

function lintCpp(): ProcessOptions {
  const cpplint = `${cwd()}/third_party/cpplint/cpplint.py`;
  const flags = [
    "--filter=-build/include_subdir",
    "--repository=src",
    "--extensions=cc,h",
    "--recursive"
  ];
  const options: RunOptions = {
    args: ["python", cpplint, ...flags, "src/."]
  };
  return ["cpplint", options];
}

function lintPython(): ProcessOptions {
  const files = findFiles({
    dir: ["tools", "build_extra"],
    ext: ".py",
    skip: ["tools/clang"]
  });
  const options: RunOptions = {
    args: ["python", "third_party/depot_tools/pylint.py", ...files]
  };
  return ["python", options];
}

function lintTS(): ProcessOptions[] {
  const tslint = `${cwd()}/third_party/node_modules/.bin/tslint`;
  const lintOptions: RunOptions = {
    args: [tslint, "-p", ".", "--exclude", "**/gen/**/*.ts"]
  };
  const lintTestOptions: RunOptions = {
    args: [
      tslint,
      "./js/**/*_test.ts",
      "./tests/**/*.ts",
      "--exclude",
      "**/gen/**/*.ts",
      "--project",
      "tsconfig.json"
    ]
  };

  return [["tslint", lintOptions], ["tslint tests", lintTestOptions]];
}

export async function lint() {
  // TODO: possibly need enable_ansi_colors()
  const processOptions: ProcessOptions[] = [
    lintCpp(),
    lintPython(),
    ...lintTS()
  ];

  const results: ProcessResult[] = await Promise.all(
    processOptions.map(resolveProcess)
  );

  // Print error messages last
  const sortedResults = results.sort(
    (a, b) => Number(b.success) - Number(a.success)
  );

  for (const { message } of sortedResults) {
    console.log(message);
  }

  if (sortedResults[sortedResults.length - 1].success !== true) {
    exit(1);
  }
}
