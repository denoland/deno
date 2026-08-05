function present(path: string): boolean {
  try {
    Deno.lstatSync(path);
    return true;
  } catch {
    return false;
  }
}
console.log(
  "deno dir:",
  present("node_modules/.deno/@denotest+libc-package-musl@1.0.0")
    ? "FOUND"
    : "NOT FOUND",
);
console.log(
  "root symlink:",
  present("node_modules/@denotest/libc-package-musl") ? "FOUND" : "NOT FOUND",
);
