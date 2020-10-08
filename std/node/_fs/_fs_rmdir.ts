type rmdirOptions = {
  maxRetries?: number;
  recursive?: boolean;
  retryDelay?: number;
};

type rmdirCallback = (err?: Error) => void;

export function rmdir(path: string | URL, callback: rmdirCallback): void;
export function rmdir(
  path: string | URL,
  options: rmdirOptions,
  callback: rmdirCallback,
): void;
export function rmdir(
  path: string | URL,
  optionsOrCallback: rmdirOptions | rmdirCallback,
  maybeCallback?: rmdirCallback,
) {
  const callback = typeof optionsOrCallback === "function"
    ? optionsOrCallback
    : maybeCallback;
  const options = typeof optionsOrCallback === "object"
    ? optionsOrCallback
    : undefined;

  if (!callback) throw new Error("No callback function supplied");

  Deno.remove(path, { recursive: options?.recursive })
    .then((_) => callback())
    .catch(callback);
}

export function rmdirSync(path: string | URL, options?: rmdirOptions) {
  Deno.removeSync(path, { recursive: options?.recursive });
}
