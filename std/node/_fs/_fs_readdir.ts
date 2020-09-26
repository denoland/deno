import { asyncIterableToCallback } from "./_fs_watch.ts";

type readDirOptions = {
  encoding?: string;
  withFileTypes?: boolean;
};

type readDirCallback = (err: Error | undefined, files: string[]) => void;

export function readdir(
  path: string | URL,
  options: readDirOptions,
  callback: readDirCallback,
): void;
export function readdir(path: string | URL, callback: readDirCallback): void;
export function readdir(
  path: string | URL,
  optionsOrCallback: readDirOptions | readDirCallback,
  maybeCallback?: readDirCallback,
) {
  const callback = typeof optionsOrCallback === "function"
    ? optionsOrCallback
    : maybeCallback;
  // const options = typeof optionsOrCallback === "object" ? optionsOrCallback : null;
  const result: string[] = [];

  if (!callback) throw new Error("No callback function supplied");

  try {
    asyncIterableToCallback(Deno.readDir(path), (val, done) => {
      if (done) {
        callback(undefined, result);
        return;
      }
      result.push(val.name);
    });
  } catch (error) {
    callback(error, result);
  }
}

// eslint-disable-next-line @typescript-eslint/no-unused-vars
export function readdirSync(path: string | URL, options?: readDirOptions) {
  const result = [];

  for (const file of Deno.readDirSync(path)) {
    result.push(file.name);
  }
  return result;
}
