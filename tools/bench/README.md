## Re-bootstrapping

Re-bootstrapping allows deno devs to bench/profile/test JS-side changes without
doing a full `cargo build --release --bin deno` which takes roughly ~4mn on M1s
more on other machines which significantly slows down iteration &
experimentation.

## Example

```js
import { benchSync, rebootstrap } from "./tools/bench/mod.js";

const bootstrap = rebootstrap([
  "webidl",
  "console",
  "url",
  "web",
  "fetch",
]);

benchSync("resp_w_h", 1e6, () =>
  new bootstrap.fetch.Response("yolo", {
    status: 200,
    headers: {
      server: "deno",
      "content-type": "text/plain",
    },
  }));
```

This code can then benched and profiled (using Chrome's DevTools) similar to
regular userland code and the original source files appear in the DevTools as
you would expect.
