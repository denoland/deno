export const version = `v${Deno.version.deno}`;

export const versions = {
  node: Deno.version.deno,
  ...Deno.version,
};

const osToPlatform = (os: Deno.OperatingSystem) =>
  os === "win" ? "win32" : os === "mac" ? "darwin" : os;

export const platform = osToPlatform(Deno.build.os);

export const { arch } = Deno.build;

export const argv = [ Deno.execPath(), ...Deno.args ];

// TODO(rsp): currently setting env seems to be working by modifying the object
// that is returnd by Deno.env(). Need to make sure that this is the final API
// or Deno.env('key', 'value') is to be used in the future.
export const env = Deno.env();

export const { pid, cwd, chdir, exit } = Deno;

export function on(event: string, callback: Function) {
  // TODO(rsp): to be implemented
  // This is needed and empty func is actually sufficient for code that do things like:
  // process.on("uncaughtException", (err) => {
  //   if (!(err instanceof ExitStatus)) throw err;
  // });
  // Deno dies on uncaught exceptions anyway, but without it it will also die on
  // registering the callback itself.
}
