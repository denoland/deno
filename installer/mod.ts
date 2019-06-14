#!/usr/bin/env deno --allow-all

const {
  args,
  env,
  readDirSync,
  mkdirSync,
  writeFile,
  exit,
  stdin,
  stat,
  readAll,
  run,
  remove
} = Deno;
import * as path from "../fs/path.ts";

const encoder = new TextEncoder();
const decoder = new TextDecoder("utf-8");

enum Permission {
  Read,
  Write,
  Net,
  Env,
  Run,
  All
}

function getPermissionFromFlag(flag: string): Permission | undefined {
  switch (flag) {
    case "--allow-read":
      return Permission.Read;
    case "--allow-write":
      return Permission.Write;
    case "--allow-net":
      return Permission.Net;
    case "--allow-env":
      return Permission.Env;
    case "--allow-run":
      return Permission.Run;
    case "--allow-all":
      return Permission.All;
    case "-A":
      return Permission.All;
  }
}

function getFlagFromPermission(perm: Permission): string {
  switch (perm) {
    case Permission.Read:
      return "--allow-read";
    case Permission.Write:
      return "--allow-write";
    case Permission.Net:
      return "--allow-net";
    case Permission.Env:
      return "--allow-env";
    case Permission.Run:
      return "--allow-run";
    case Permission.All:
      return "--allow-all";
  }
  return "";
}

async function readCharacter(): Promise<string> {
  const byteArray = new Uint8Array(1024);
  await stdin.read(byteArray);
  const line = decoder.decode(byteArray);
  return line[0];
}

async function yesNoPrompt(message: string): Promise<boolean> {
  console.log(`${message} [yN]`);
  const input = await readCharacter();
  console.log();
  return input === "y" || input === "Y";
}

function createDirIfNotExists(path: string): void {
  try {
    readDirSync(path);
  } catch (e) {
    mkdirSync(path, true);
  }
}

function checkIfExistsInPath(path: string): boolean {
  const { PATH } = env();

  const paths = (PATH as string).split(":");

  return paths.includes(path);
}

function getInstallerDir(): string {
  const { HOME } = env();

  if (!HOME) {
    throw new Error("$HOME is not defined.");
  }

  return path.join(HOME, ".deno", "bin");
}

// TODO: fetch doesn't handle redirects yet - once it does this function
//  can be removed
async function fetchWithRedirects(
  url: string,
  redirectLimit: number = 10
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
): Promise<any> {
  // TODO: `Response` is not exposed in global so 'any'
  const response = await fetch(url);

  if (response.status === 301 || response.status === 302) {
    if (redirectLimit > 0) {
      const redirectUrl = response.headers.get("location")!;
      return await fetchWithRedirects(redirectUrl, redirectLimit - 1);
    }
  }

  return response;
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
async function fetchModule(url: string): Promise<any> {
  const response = await fetchWithRedirects(url);

  if (response.status !== 200) {
    // TODO: show more debug information like status and maybe body
    throw new Error(`Failed to get remote script ${url}.`);
  }

  const body = await readAll(response.body);
  return decoder.decode(body);
}

function showHelp(): void {
  console.log(`deno installer
  Install remote or local script as executables.
  
USAGE:
  deno https://deno.land/std/installer/mod.ts EXE_NAME SCRIPT_URL [FLAGS...]  

ARGS:
  EXE_NAME  Name for executable     
  SCRIPT_URL  Local or remote URL of script to install
  [FLAGS...]  List of flags for script, both Deno permission and script specific flag can be used.
  `);
}

export async function install(
  moduleName: string,
  moduleUrl: string,
  flags: string[]
): Promise<void> {
  const installerDir = getInstallerDir();
  createDirIfNotExists(installerDir);

  const FILE_PATH = path.join(installerDir, moduleName);

  let fileInfo;
  try {
    fileInfo = await stat(FILE_PATH);
  } catch (e) {
    // pass
  }

  if (fileInfo) {
    const msg = `⚠️  ${moduleName} is already installed, do you want to overwrite it?`;
    if (!(await yesNoPrompt(msg))) {
      return;
    }
  }

  // ensure script that is being installed exists
  if (moduleUrl.startsWith("http")) {
    // remote module
    console.log(`Downloading: ${moduleUrl}\n`);
    await fetchModule(moduleUrl);
  } else {
    // assume that it's local file
    moduleUrl = path.resolve(moduleUrl);
    console.log(`Looking for: ${moduleUrl}\n`);
    await stat(moduleUrl);
  }

  const grantedPermissions: Permission[] = [];
  const scriptArgs: string[] = [];

  for (const flag of flags) {
    const permission = getPermissionFromFlag(flag);
    if (permission === undefined) {
      scriptArgs.push(flag);
    } else {
      grantedPermissions.push(permission);
    }
  }

  const commands = [
    "deno",
    ...grantedPermissions.map(getFlagFromPermission),
    moduleUrl,
    ...scriptArgs,
    "$@"
  ];

  // TODO: add windows Version
  const template = `#/bin/sh\n${commands.join(" ")}`;
  await writeFile(FILE_PATH, encoder.encode(template));

  const makeExecutable = run({ args: ["chmod", "+x", FILE_PATH] });
  const { code } = await makeExecutable.status();
  makeExecutable.close();

  if (code !== 0) {
    throw new Error("Failed to make file executable");
  }

  console.log(`✅ Successfully installed ${moduleName}.`);
  // TODO: add Windows version
  if (!checkIfExistsInPath(installerDir)) {
    console.log("\nℹ️  Add ~/.deno/bin to PATH");
    console.log(
      "    echo 'export PATH=\"$HOME/.deno/bin:$PATH\"' >> ~/.bashrc # change this to your shell"
    );
  }
}

export async function uninstall(moduleName: string): Promise<void> {
  const installerDir = getInstallerDir();
  const FILE_PATH = path.join(installerDir, moduleName);

  try {
    await stat(FILE_PATH);
  } catch (e) {
    if (e instanceof Deno.DenoError && e.kind === Deno.ErrorKind.NotFound) {
      throw new Error(`ℹ️  ${moduleName} not found`);
    }
  }

  await remove(FILE_PATH);
  console.log(`ℹ️  Uninstalled ${moduleName}`);
}

async function main(): Promise<void> {
  if (args.length < 3) {
    return showHelp();
  }

  if (["-h", "--help"].includes(args[1])) {
    return showHelp();
  }

  const moduleName = args[1];
  const moduleUrl = args[2];
  const flags = args.slice(3);
  try {
    await install(moduleName, moduleUrl, flags);
  } catch (e) {
    console.log(e);
    exit(1);
  }
}

if (import.meta.main) {
  main();
}
