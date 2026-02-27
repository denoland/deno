// Copyright 2018-2025 the Deno authors. MIT license.
// https://github.com/dsherret/deno-which/blob/main/mod.ts

/**
 * This file triggered a crash in Deno.
 */

declare namespace Deno {
  class FileInfo {
    name: string;
    isFile: boolean;
  }
  function stat(...any);
  function statSync(...any);
  namespace env {
    function get(string);
  }
  namespace errors {
    class NotCapable {}
  }
  namespace build {
    const os: string;
  }
}

export interface Environment {
  /** Gets an environment variable. */
  env(key: string): string | undefined;
  /** Resolves the `Deno.FileInfo` for the specified
   * path following symlinks.
   */
  stat(filePath: string): Promise<Pick<Deno.FileInfo, "isFile">>;
  /** Synchronously resolves the `Deno.FileInfo` for
   * the specified path following symlinks.
   */
  statSync(filePath: string): Pick<Deno.FileInfo, "isFile">;
  /** Gets the current operating system. */
  os: typeof Deno.build.os;
}

export class RealEnvironment implements Environment {
  env(key: string): string | undefined {
    return Deno.env.get(key);
  }

  stat(path: string): Promise<Pick<Deno.FileInfo, "isFile">> {
    return Deno.stat(path);
  }

  statSync(path: string): Pick<Deno.FileInfo, "isFile"> {
    return Deno.statSync(path);
  }

  get os(): typeof Deno.build.os {
    return Deno.build.os;
  }
}

/** Finds the path to the specified command asynchronously. */
export async function which(
  command: string,
  environment: Omit<Environment, "statSync"> = new RealEnvironment(),
): Promise<string | undefined> {
  const systemInfo = getSystemInfo(command, environment);
  if (systemInfo == null) {
    return undefined;
  }

  for (const pathItem of systemInfo.pathItems) {
    const filePath = pathItem + command;
    if (systemInfo.pathExts) {
      for (const pathExt of systemInfo.pathExts) {
        const filePath = pathItem + command + pathExt;
        if (await pathMatches(environment, filePath)) {
          return filePath;
        }
      }
    } else {
      if (await pathMatches(environment, filePath)) {
        return filePath;
      }
    }
  }

  return undefined;
}

async function pathMatches(
  environment: Omit<Environment, "statSync">,
  path: string,
): Promise<boolean> {
  try {
    const result = await environment.stat(path);
    return result.isFile;
  } catch (err) {
    if (err instanceof Deno.errors.NotCapable) {
      throw err;
    }
    return false;
  }
}

/** Finds the path to the specified command synchronously. */
export function whichSync(
  command: string,
  environment: Omit<Environment, "stat"> = new RealEnvironment(),
): string | undefined {
  const systemInfo = getSystemInfo(command, environment);
  if (systemInfo == null) {
    return undefined;
  }

  for (const pathItem of systemInfo.pathItems) {
    const filePath = pathItem + command;
    if (pathMatchesSync(environment, filePath)) {
      return filePath;
    }
    if (systemInfo.pathExts) {
      for (const pathExt of systemInfo.pathExts) {
        const filePath = pathItem + command + pathExt;
        if (pathMatchesSync(environment, filePath)) {
          return filePath;
        }
      }
    }
  }

  return undefined;
}

function pathMatchesSync(
  environment: Omit<Environment, "stat">,
  path: string,
): boolean {
  try {
    const result = environment.statSync(path);
    return result.isFile;
  } catch (err) {
    if (err instanceof Deno.errors.NotCapable) {
      throw err;
    }
    return false;
  }
}

interface SystemInfo {
  pathItems: string[];
  pathExts: string[] | undefined;
  isNameMatch: (a: string, b: string) => boolean;
}

function getSystemInfo(
  command: string,
  environment: Omit<Environment, "stat" | "statSync">,
): SystemInfo | undefined {
  const isWindows = environment.os === "windows";
  const envValueSeparator = isWindows ? ";" : ":";
  const path = environment.env("PATH");
  const pathSeparator = isWindows ? "\\" : "/";
  if (path == null) {
    return undefined;
  }

  return {
    pathItems: splitEnvValue(path).map((item) => normalizeDir(item)),
    pathExts: getPathExts(),
    isNameMatch: isWindows
      ? (a, b) => a.toLowerCase() === b.toLowerCase()
      : (a, b) => a === b,
  };

  function getPathExts() {
    if (!isWindows) {
      return undefined;
    }

    const pathExtText = environment.env("PATHEXT") ?? ".EXE;.CMD;.BAT;.COM";
    const pathExts = splitEnvValue(pathExtText);
    const lowerCaseCommand = command.toLowerCase();

    for (const pathExt of pathExts) {
      // Do not use the pathExts if someone has provided a command
      // that ends with the extenion of an executable extension
      if (lowerCaseCommand.endsWith(pathExt.toLowerCase())) {
        return undefined;
      }
    }

    return pathExts;
  }

  function splitEnvValue(value: string) {
    return value
      .split(envValueSeparator)
      .map((item) => item.trim())
      .filter((item) => item.length > 0);
  }

  function normalizeDir(dirPath: string) {
    if (!dirPath.endsWith(pathSeparator)) {
      dirPath += pathSeparator;
    }
    return dirPath;
  }
}
