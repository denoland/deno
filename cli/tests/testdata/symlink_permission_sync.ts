const oldname = Deno.args[0];
const newname = Deno.args[1];
Deno.symlinkSync(oldname, newname);
