## Program lifecycle

Deno supports browser compatible lifecycle events: `load` and `unload`. You can
use these events to provide setup and cleanup code in your program.

Listeners for `load` events can be asynchronous and will be awaited. Listeners
for `unload` events need to be synchronous. Both events cannot be cancelled.

Example:

**main.ts**

```ts
import "./imported.ts";

const handler = (e: Event): void => {
  console.log(`got ${e.type} event in event handler (main)`);
};

window.addEventListener("load", handler);

window.addEventListener("unload", handler);

window.onload = (e: Event): void => {
  console.log(`got ${e.type} event in onload function (main)`);
};

window.onunload = (e: Event): void => {
  console.log(`got ${e.type} event in onunload function (main)`);
};

console.log("log from main script");
```

**imported.ts**

```ts
const handler = (e: Event): void => {
  console.log(`got ${e.type} event in event handler (imported)`);
};

window.addEventListener("load", handler);
window.addEventListener("unload", handler);

window.onload = (e: Event): void => {
  console.log(`got ${e.type} event in onload function (imported)`);
};

window.onunload = (e: Event): void => {
  console.log(`got ${e.type} event in onunload function (imported)`);
};

console.log("log from imported script");
```

Note that you can use both `window.addEventListener` and
`window.onload`/`window.onunload` to define handlers for events. There is a
major difference between them, let's run the example:

```shell
$ deno run main.ts
log from imported script
log from main script
got load event in event handler (imported)
got load event in event handler (main)
got load event in onload function (main)
got unload event in event handler (imported)
got unload event in event handler (main)
got unload event in onunload function (main)
```

All listeners added using `window.addEventListener` were run, but
`window.onload` and `window.onunload` defined in `main.ts` overrode handlers
defined in `imported.ts`.

In other words, you can register multiple `window.addEventListener` `"load"` or
`"unload"` events, but only the last loaded `window.onload` or `window.onunload`
event handlers will be executed. It is preferable to use `addEventListener`
when possible for this reason.