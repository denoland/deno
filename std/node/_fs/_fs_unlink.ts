export function unlink(path: string | URL, callback: (err?: Error) => any) {
  Deno.remove(path)
    .then((_) => callback())
    .catch(callback);
}

export function unlinkSync(path: string | URL) {
  Deno.removeSync(path);
}
