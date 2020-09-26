export function unlink(path: string | URL, callback: (err?: Error) => void) {
  Deno.remove(path)
    .then((_) => callback())
    .catch(callback);
}

export function unlinkSync(path: string | URL) {
  Deno.removeSync(path);
}
