Deno.env.get("FOO");
Deno.env.get("BAR");

Deno.loadavg();
Deno.hostname();

Deno.cwd();
Deno.lstatSync(new URL("../", import.meta.url));
