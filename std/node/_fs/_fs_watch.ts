import { fromFileUrl } from "../path.ts";

export function asyncIterableIteratorToCallback<T>(
  iterator: AsyncIterableIterator<T>,
  callback: (val: T, done?: boolean) => void,
) {
  function next() {
    iterator.next().then((obj) => {
      if (obj.done) {
        // @ts-ignore
        callback(undefined, true);
        return;
      }
      callback(obj.value);
      next();
    });
  }
  next();
}

export function asyncIterableToCallback<T>(
  iter: AsyncIterable<T>,
  callback: (val: T, done?: boolean) => void,
) {
  const iterator = iter[Symbol.asyncIterator]();
  function next() {
    iterator.next().then((obj) => {
      if (obj.done) {
        // @ts-ignore
        callback(undefined, true);
        return;
      }
      callback(obj.value);
      next();
    });
  }
  next();
}

type watchOptions = {
  persistent: boolean;
  recursive: boolean;
  encoding: string;
};

type watchListener = (eventType: string, filename: string) => void;

export function watch(
  filename: string | URL,
  options: watchOptions,
  listener: watchListener,
): { close: () => void };
export function watch(
  filename: string | URL,
  listener: watchListener,
): { close: () => void };
export function watch(
  filename: string | URL,
  options: watchOptions,
): { close: () => void };
export function watch(filename: string | URL): { close: () => void };
export function watch(
  filename: string | URL,
  optionsOrListener?: watchOptions | watchListener,
  optionsOrListener2?: watchOptions | watchListener,
) {
  const listener = typeof optionsOrListener === "function"
    ? optionsOrListener
    : typeof optionsOrListener2 === "function"
    ? optionsOrListener2
    : undefined;
  const options = typeof optionsOrListener === "object"
    ? optionsOrListener
    : typeof optionsOrListener2 === "object"
    ? optionsOrListener2
    : undefined;
  filename = filename instanceof URL ? fromFileUrl(filename) : filename;

  const iterator = Deno.watchFs(filename, {
    recursive: options?.recursive || false,
  });

  if (!listener) throw new Error("No callback function supplied");

  asyncIterableIteratorToCallback<Deno.FsEvent>(iterator, (val) => {
    listener(val.kind, val.paths[0]);
  });

  return {
    close() {
      // @ts-ignore
      iterator.return();
    },
  };
}
