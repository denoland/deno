// `folder/linkdir` symlinks out of the included tree, so the include walker
// reaches `skip.ts` as `folder/linkdir/skip.ts` while the VFS builder reaches
// it as `real/skip.ts`.
Deno.symlinkSync("../real", "folder/linkdir", { type: "dir" });
