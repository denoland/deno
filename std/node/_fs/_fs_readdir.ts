type readDirOptions = {
  encoding: string;
  withFileTypes: boolean;
};

type readDirCallback = (err: Error, files: string[]) => any;

export function readdir(
  path: string | URL,
  options: readDirOptions,
  callback: readDirCallback
): void;
export function readdir(path: string | URL, callback: readDirCallback): void;
export function readdir(
  path: string | URL,
  optionsOrCallback: readDirOptions | readDirCallback,
  maybeCallback?: readDirCallback
) {
  const callback =
    typeof optionsOrCallback === "function"
      ? optionsOrCallback
      : maybeCallback || (() => {});
  // const options = typeof optionsOrCallback === "object" ? optionsOrCallback : null;
  const result = [];

  try {
    for (let file of Deno.readDirSync(path)) {
      result.push(file.name);
    }
  } catch (error) {
    callback(error, result);
  }
}

export function readdirSync(path: string | URL, options?: readDirOptions) {
  const result = [];

  for (let file of Deno.readDirSync(path)) {
    result.push(file.name);
  }
  return result;
}
