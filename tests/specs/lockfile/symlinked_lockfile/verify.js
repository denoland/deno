// `deno add` must write through the symlink: the `deno.lock` symlink should be
// preserved and the real target file should receive the update, instead of the
// symlink being clobbered with a fresh regular file.
console.log("deno.lock is symlink:", Deno.lstatSync("deno.lock").isSymlink);
const target = Deno.readTextFileSync("real/deno.lock");
console.log("target updated:", target.includes("@denotest/add"));
