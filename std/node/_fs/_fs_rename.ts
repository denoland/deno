import { fromFileUrl } from "../path.ts";

export function rename(
  oldPath: string | URL,
  newPath: string | URL,
  callback: (err?: Error) => any
) {
  oldPath = oldPath instanceof URL ? fromFileUrl(oldPath) : oldPath;
  newPath = newPath instanceof URL ? fromFileUrl(newPath) : newPath;

  Deno.rename(oldPath, newPath)
    .then((_) => callback())
    .catch(callback);
}

export function renameSync(oldPath: string | URL, newPath: string | URL) {
  oldPath = oldPath instanceof URL ? fromFileUrl(oldPath) : oldPath;
  newPath = newPath instanceof URL ? fromFileUrl(newPath) : newPath;

  Deno.renameSync(oldPath, newPath);
}
