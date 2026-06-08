function present(pkg: string): boolean {
  try {
    Deno.statSync(`node_modules/.deno/${pkg}@1.0.0`);
    return true;
  } catch {
    return false;
  }
}
console.log(
  "glibc:",
  present("@denotest+libc-package-glibc") ? "FOUND" : "NOT FOUND",
);
console.log(
  "musl:",
  present("@denotest+libc-package-musl") ? "FOUND" : "NOT FOUND",
);
