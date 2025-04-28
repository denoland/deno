const [from, to] = Deno.args;

Deno.renameSync(from.trim(), to.trim());
