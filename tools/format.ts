import { cwd, exit, platform, RunOptions } from "deno";
import {
  findFiles,
  ProcessOptions,
  resolveProcess,
  ProcessResult
} from "./third_party.ts";

function formatClang(): ProcessOptions {
  const executable = `${cwd()}/third_party/depot_tools/clang-format`;
  const flags = ["-i", "-style", "Google"];
  const clangFiles = [...findFiles({ dir: "libdeno", ext: [".cc", ".h"] })];
  const options: RunOptions = {
    args: [executable, ...flags, ...clangFiles]
  };
  return ["clang", options];
}

function formatGN(): ProcessOptions[] {
  const executable = "third_party/depot_tools/gn";

  const gnFiles = [
    "BUILD.gn",
    ".gn",
    ...findFiles({ dir: ["build_extra", "libdeno"], ext: [".gn", ".gni"] })
  ];

  // TODO: google_env() equivalent (eg in third_party.py)
  const processOptions = [];
  for (const filename of gnFiles) {
    processOptions.push([
      `gn ${filename}`,
      {
        args: [executable, "format", filename]
      }
    ]);
  }

  return processOptions;
}

function formatPython(): ProcessOptions {
  const executable = "python";
  const yapfPath = "third_party/python_packages/bin/yapf";

  const pythonFiles = findFiles({
    dir: ["tools", "build_extra"],
    ext: ".py",
    skip: ["tools/clang"]
  });
  // TODO: python_env() equivalent (eg in third_party.py)
  const options: RunOptions = {
    args: [executable, yapfPath, "-i", ...pythonFiles]
  };

  return ["yapf", options];
}

function formatPrettier(): ProcessOptions {
  const executable = `${cwd()}/third_party/node_modules/.bin/prettier`;

  const prettierFiles = [
    ...findFiles({
      dir: cwd(),
      ext: [".json", ".md", ".js"],
      depth: 0
    }),
    ...findFiles({
      dir: [".github", "js", "tests", "tools", "website"],
      ext: [".js", ".json", ".ts", ".md"],
      skip: ["tools/clang", "js/deps"]
    })
  ];
  const flags = ["--write" /* "--loglevel=error" */];
  const options: RunOptions = {
    args: [executable, ...flags, ...prettierFiles]
  };

  return ["prettier", options];
}

function formatRust(): ProcessOptions {
  const toolsPath = `${cwd()}/tools`;
  const executable = `third_party/rustfmt/${platform.os}/rustfmt`;
  const flags = ["--config-path", `${toolsPath}/rustfmt.toml`];
  const rustFilepaths = ["build.rs", ...findFiles({ dir: "src", ext: ".rs" })];
  const options: RunOptions = {
    args: [executable, ...flags, ...rustFilepaths]
  };
  return ["rustfmt", options];
}

export async function format() {
  // TODO: we might need a fix_symlinks() equivalent (eg in third_party.py).

  const processOptions: ProcessOptions[] = [
    formatClang(),
    ...formatGN(),
    formatPython(),
    formatPrettier(),
    formatRust()
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

export async function testFormat() {
  // Format code
  console.log("Format:");
  await format();

  // Check for git changes
  const { message, success, stdout } = await resolveProcess([
    "git status",
    {
      args: ["git", "status", "-uno", "--porcelain", "--ignore-submodules"]
    }
  ]);

  console.log("Git changes:");
  console.log(message);

  if (!success) {
    exit(1);
  }
  if (await stdout) {
    console.log("✖ validate no changes");
    console.log(await stdout);
    exit(1);
  }
  console.log("✔ validdate no changes");
}
