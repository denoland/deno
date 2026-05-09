import { register } from "node:module";

// Regression test: importing a node: builtin after register() used to
// panic because the builtin's ext: dependencies were routed through the
// async hook resolve bridge during V8's synchronous module instantiation.
register("./hooks-basic.mjs", import.meta.url);

const os = await import("node:os");
console.log(typeof os.EOL);
