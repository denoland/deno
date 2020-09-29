const foo = await import("./test.js");

console.log(foo);

// before sleep
// error: Uncaught ReferenceError: default is not defined
//     at Function.keys (<anonymous>)
//     at inspectRawObject (deno:cli/rt/02_console.js:698:31)
//     at inspectObject (deno:cli/rt/02_console.js:801:14)
//     at inspectValue (deno:cli/rt/02_console.js:429:16)
//     at inspectArgs (deno:cli/rt/02_console.js:1249:17)
//     at Object.log (deno:cli/rt/02_console.js:1284:9)
//     at file:///Users/biwanczuk/dev/deno/test1.js:3:9
