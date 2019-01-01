import { Process, readAll, readDirSync, run, RunOptions } from "deno";

export interface FindOptions {
  dir: string[] | string;
  ext: string[] | string;
  skip?: string[];
  depth?: number;
}

export function findFiles({
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

export type ProcessOptions = [string, RunOptions];
export type ProcessResult = {
  success: boolean;
  message: string;
};

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

export async function resolveProcess([name, runOpts]: ProcessOptions): Promise<
  ProcessResult
> {
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
