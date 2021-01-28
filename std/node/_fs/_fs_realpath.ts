type Options = { encoding: string };
type Callback = (err: Error | null, path?: string) => void;

export function realpath(
  path: string,
  options?: Options | Callback,
  callback?: Callback,
) {
  if (typeof options === "function") {
    callback = options;
  }
  if (!callback) {
    throw new Error("No callback function supplied");
  }
  Deno.realPath(path).then(
    (path) => callback!(null, path),
    (err) => callback!(err),
  );
}

export function realpathSync(path: string): string {
  return Deno.realPathSync(path);
}
