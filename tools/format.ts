#!/usr/bin/env deno --allow-run
import {
  cwd,
  exit,
  platform,
  Process,
  ProcessStatus,
  readAll,
  readDirSync,
  run,
  RunOptions
} from "deno";

// TODO: we might need a fix_symlinks() equivalent (eg in third_party.py).

type ProcessOptions = [string, RunOptions];

function runp(runOpts: RunOptions) {
  return run(
    Object.assign(
      {
        stdout: "piped",
        stderr: "piped"
      },
      runOpts
    )
  );
}

function pathJoin(...args: string[]): string {
  return args.join("/");
}

interface FindOptions {
  dir: string[] | string;
  ext: string[] | string;
  skip?: string[];
  depth?: number;
}

function findFiles({
  dir,
  ext,
  skip = [],
  depth = Infinity
}: FindOptions): string[] {
  const dirs = typeof dir === "string" ? [dir] : dir;
  const exts: string[] = typeof ext === "string" ? [ext] : ext;
  const filesByPath = dirs.map(path =>
    findFilesWalk({ path, exts, skip, depth })
  );
  const files = filesByPath.reduce((acc, item) => acc.concat(item), []);
  return files;
}

function findFilesWalk({ path, exts = [], skip = [], depth }): string[] {
  const matchedFiles = [];
  const files = readDirSync(path);
  files.forEach(file => {
    const isDirectory = file.isDirectory();
    // Only search recursively to the given depth
    if (isDirectory && depth < 1) {
      return;
    }
    // Ignore directories based on skip argument
    if (isDirectory && skip.find(skipStr => file.path.endsWith(skipStr))) {
      return;
    }
    // Ignore files based on skip argument
    if (!isDirectory && skip.find(skipStr => file.name === skipStr)) {
      return;
    }
    if (isDirectory) {
      const paths = findFilesWalk({
        path: file.path,
        exts,
        skip,
        depth: depth - 1
      });
      matchedFiles.concat(paths);
      return;
    }

    for (const ext of exts) {
      if (file.name.endsWith(ext)) {
        matchedFiles.push(file.path);
        return;
      }
    }
  });

  return matchedFiles;
}

/* Run Formatting */

function formatClang(): ProcessOptions {
  const executable = pathJoin(
    cwd(),
    "third_party",
    "depot_tools",
    "clang-format"
  );
  const flags = ["-i", "-style", "Google"];
  const clangFiles = [...findFiles({ dir: "libdeno", ext: [".cc", ".h"] })];
  const options: RunOptions = {
    args: [executable, ...flags, ...clangFiles]
  };
  return ["clang", options];
}

function formatGN(): ProcessOptions[] {
  const executable = pathJoin("third_party", "depot_tools", "gn");

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
  const yapfPath = pathJoin("third_party", "python_packages", "bin", "yapf");

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
  const executable = pathJoin(
    cwd(),
    "third_party",
    "node_modules",
    ".bin",
    "prettier"
  );

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
  const toolsPath = pathJoin(cwd(), "tools");
  const executable = pathJoin("third_party", "rustfmt", platform.os, "rustfmt");
  const flags = ["--config-path", pathJoin(toolsPath, "rustfmt.toml")];
  const rustFilepaths = ["build.rs", ...findFiles({ dir: "src", ext: ".rs" })];
  const options: RunOptions = {
    args: [executable, ...flags, ...rustFilepaths]
  };
  return ["rustfmt", options];
}

async function resolveProcess([name, runOpts]: ProcessOptions) {
  const process: Process = runp(runOpts);
  const status = await process.status();
  const failed = status.code !== 0 || status.success !== true;

  if (failed) {
    const stdout = await readAll(process.stdout);
    const stderr = await readAll(process.stderr);
    const decoder = new TextDecoder("utf-8");
    const message = [
      `✖ ${name}`,
      decoder.decode(stdout).trim(),
      decoder.decode(stderr).trim()
    ]
      .filter(Boolean)
      .join("\n");

    return {
      success: false,
      message
    };
  }
  return {
    success: true,
    message: `✔ ${name}`
  };
}

async function main() {
  const processOptions: ProcessOptions[] = [
    formatClang(),
    ...formatGN(),
    formatPython(),
    formatPrettier(),
    formatRust()
  ];

  const results = await Promise.all(processOptions.map(resolveProcess));

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

main();
