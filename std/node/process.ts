import { notImplemented } from "./_utils.ts";

const version = `v${Deno.version.deno}`;

const versions = {
  node: Deno.version.deno,
  ...Deno.version,
};

const osToPlatform = (os: Deno.OperatingSystem): string =>
  os === "win" ? "win32" : os === "mac" ? "darwin" : os;

const platform = osToPlatform(Deno.build.os);

const { arch } = Deno.build;

const { pid, cwd, chdir, exit } = Deno;

function on(_event: string, _callback: Function): void {
  // TODO(rsp): to be implemented
  notImplemented();
}

export const process = {
  version,
  versions,
  platform,
  arch,
  pid,
  cwd,
  chdir,
  exit,
  on,
  get env(): { [index: string]: string } {
    // using getter to avoid --allow-env unless it's used
    return Deno.env();
  },
  get argv(): string[] {
    // Deno.execPath() also requires --allow-env
    return [Deno.execPath(), ...Deno.args];
  },
};
