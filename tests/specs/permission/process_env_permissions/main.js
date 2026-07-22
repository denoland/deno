console.log(Deno.env.get("MYAPP_HELLO"));
console.log(Deno.env.get("MYAPP_GOODBYE"));
Deno.env.set("MYAPP_TEST", "done");
Deno.env.set("MYAPP_DONE", "done");
console.log(Deno.env.get("MYAPP_DONE"));
