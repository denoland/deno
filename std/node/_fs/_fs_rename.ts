import { fromFileUrl } from "../path.ts";

export function rename(
  oldPath: string | URL,
  newPath: string | URL,
  callback: (err?: Error) => void,
) {
  oldPath = oldPath instanceof URL ? fromFileUrl(oldPath) : oldPath;
  newPath = newPath instanceof URL ? fromFileUrl(newPath) : newPath;

  if (!callback) throw new Error("No callback function supplied");

  Deno.rename(oldPath, newPath).then((_) => callback(), callback);
}

export function renameSync(oldPath: string | URL, newPath: string | URL) {
  oldPath = oldPath instanceof URL ? fromFileUrl(oldPath) : oldPath;
  newPath = newPath instanceof URL ? fromFileUrl(newPath) : newPath;

  Deno.renameSync(oldPath, newPath);
}
