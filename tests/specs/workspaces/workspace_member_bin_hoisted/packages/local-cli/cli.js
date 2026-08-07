#!/usr/bin/env node
// `deno task` runs this itself (`deno run --ext=js`) rather than handing it to
// whichever `node` happens to be on `PATH`, so a workspace member's executable
// works without Node installed. Print the runtime to lock that in.
console.log("Hello from", typeof Deno === "undefined" ? "node" : "deno");
