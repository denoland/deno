export const version = `v${Deno.version.deno}`;

export const versions = {
  node: Deno.version.deno,
  ...Deno.version
};

const osToPlatform = (os: Deno.OperatingSystem): string =>
  os === "win" ? "win32" : os === "mac" ? "darwin" : os;

export const platform = osToPlatform(Deno.build.os);

export const { arch } = Deno.build;

export const argv = [Deno.execPath(), ...Deno.args];

// TODO(rsp): currently setting env seems to be working by modifying the object
// that is returnd by Deno.env(). Need to make sure that this is the final API
// or Deno.env('key', 'value') is to be used in the future.
export const env = Deno.env();

export const { pid, cwd, chdir, exit } = Deno;

export function on(_event: string, _callback: Function): void {
  // TODO(rsp): to be implemented
  throw Error("unimplemented");
}
