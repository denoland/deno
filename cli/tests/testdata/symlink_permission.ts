const oldname = Deno.args[0];
const newname = Deno.args[1];
await Deno.symlink(oldname, newname);
