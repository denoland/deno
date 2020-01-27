#!/usr/bin/env -S deno --allow-all
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
const { env, args, exit } = Deno;
import { parse } from "../flags/mod.ts";
import { exists } from "../fs/exists.ts";
import { ensureDir } from "../fs/ensure_dir.ts";
import * as path from "../path/mod.ts";

function showHelp(): void {
  console.log(`deno uninstaller
  Uninstall previously installed executables.

USAGE:
  deno -A https://deno.land/std/installer/uninstall.ts [OPTIONS] EXE_NAME

ARGS:
  EXE_NAME  Name for executable

OPTIONS:
  -d, --dir <PATH> Installation directory path (defaults to ~/.deno/bin)
`);
}

function getInstallerDir(): string {
  // In Windows's Powershell $HOME environmental variable maybe null
  // if so use $USERPROFILE instead.
  const { HOME, USERPROFILE } = env();

  const HOME_PATH = HOME || USERPROFILE;

  if (!HOME_PATH) {
    throw new Error("$HOME is not defined.");
  }

  return path.resolve(HOME_PATH, ".deno", "bin");
}

function validateModuleName(moduleName: string): boolean {
  if (/^[a-z][\w-]*$/i.test(moduleName)) {
    return true;
  } else {
    throw new Error("Invalid module name: " + moduleName);
  }
}

export async function uninstall(
  executableName: string,
  installationDir?: string
): Promise<void> {
  if (!installationDir) {
    installationDir = getInstallerDir();
  }
  await ensureDir(installationDir);
  validateModuleName(executableName);

  const filePath = path.join(installationDir, executableName);

  if (!(await exists(filePath))) {
    console.log(
      `No installed '${executableName}' found under ${installationDir}`
    );
    return;
  }

  await Deno.remove(filePath);
  console.log(`✅ Successfully uninstalled ${executableName}`);
}

async function main(): Promise<void> {
  const parsedArgs = parse(args, { stopEarly: true });

  if (parsedArgs.h || parsedArgs.help) {
    return showHelp();
  }

  if (parsedArgs._.length < 1) {
    return showHelp();
  }

  const moduleName = parsedArgs._[0];
  const installationDir = parsedArgs.d || parsedArgs.dir;

  try {
    await uninstall(moduleName, installationDir);
  } catch (e) {
    console.log(e);
    exit(1);
  }
}

if (import.meta.main) {
  main();
}
