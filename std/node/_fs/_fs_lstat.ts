import {
  statCallback,
  statOptions,
  convertFileInfoToStats,
  Stats,
} from "./_fs_stat.ts";

export function lstat(
  path: string | URL,
  options: statOptions,
  callback: statCallback
): void;
export function lstat(path: string | URL, callback: statCallback): void;
export function lstat(
  path: string | URL,
  optionsOrCallback: statCallback | statOptions,
  maybeCallback?: statCallback
) {
  const callback =
    typeof optionsOrCallback === "function"
      ? optionsOrCallback
      : maybeCallback || (() => {});
  // const options =
  //  typeof optionsOrCallback === "object" ? optionsOrCallback : null;
  Deno.lstat(path)
    .then((stat) => callback(undefined, convertFileInfoToStats(stat)))
    // @ts-ignore
    .catch((err) => callback(err, null));
}

export function lstatSync(path: string | URL, options?: statOptions): Stats {
  const origin = Deno.lstatSync(path);
  return convertFileInfoToStats(origin);
}
