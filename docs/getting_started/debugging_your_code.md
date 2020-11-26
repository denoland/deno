## Debugging your code

Deno supports the [V8 Inspector Protocol](https://v8.dev/docs/inspector).

It's possible to debug Deno programs using Chrome Devtools or other clients that
support the protocol (eg. VSCode).

To activate debugging capabilities run Deno with the `--inspect` or
`--inspect-brk` flags.

The `--inspect` flag allows attaching the debugger at any point in time, while
`--inspect-brk` will wait for the debugger to attach and will pause execution on
the first line of code.

### Chrome Devtools

Let's try debugging a program using Chrome Devtools. For this, we'll use
[file_server.ts](https://deno.land/std@$STD_VERSION/http/file_server.ts) from
`std`, a static file server.

Use the `--inspect-brk` flag to break execution on the first line:

```shell
$ deno run --inspect-brk --allow-read --allow-net https://deno.land/std@$STD_VERSION/http/file_server.ts
Debugger listening on ws://127.0.0.1:9229/ws/1e82c406-85a9-44ab-86b6-7341583480b1
Download https://deno.land/std@$STD_VERSION/http/file_server.ts
Compile https://deno.land/std@$STD_VERSION/http/file_server.ts
...
```

Open `chrome://inspect` and click `Inspect` next to target:

![chrome://inspect](../images/debugger1.jpg)

It might take a few seconds after opening the Devtools to load all modules.

![Devtools opened](../images/debugger2.jpg)

You might notice that Devtools paused execution on the first line of
`_constants.ts` instead of `file_server.ts`. This is expected behavior and is
caused by the way ES modules are evaluated by V8 (`_constants.ts` is left-most,
bottom-most dependency of `file_server.ts` so it is evaluated first).

At this point all source code is available in the Devtools, so let's open up
`file_server.ts` and add a breakpoint there; go to "Sources" pane and expand the
tree:

![Open file_server.ts](../images/debugger3.jpg)

_Looking closely you'll find duplicate entries for each file; one written
regularly and one in italics. The former is compiled source file (so in the case
of `.ts` files it will be emitted JavaScript source), while the latter is a
source map for the file._

Next, add a breakpoint in the `listenAndServe` method:

![Break in file_server.ts](../images/debugger4.jpg)

As soon as we've added the breakpoint Devtools automatically opened up the
source map file, which allows us step through the actual source code that
includes types.

Now that we have our breakpoints set, we can resume the execution of our script
so that we might inspect an incoming request. Hit the Resume script execution
button to do so. You might even need to hit it twice!

Once our script is running again, let's send a request and inspect it in
Devtools:

```
$ curl http://0.0.0.0:4507/
```

![Break in request handling](../images/debugger5.jpg)

At this point we can introspect the contents of the request and go step-by-step
to debug the code.

### VSCode

Deno can be debugged using VSCode.

Official support via the plugin is being worked on -
https://github.com/denoland/vscode_deno/issues/12

We can still attach the debugger by manually providing a
[`launch.json`](https://code.visualstudio.com/docs/editor/debugging#_launch-configurations)
config:

```json
{
  "version": "0.2.0",
  "configurations": [
    {
      "name": "Deno",
      "type": "pwa-node",
      "request": "launch",
      "cwd": "${workspaceFolder}",
      "runtimeExecutable": "deno",
      "runtimeArgs": ["run", "--inspect-brk", "-A", "${file}"],
      "attachSimplePort": 9229
    }
  ]
}
```

**NOTE**: This uses the file you have open as the entry point; replace `${file}`
with a script name if you want a fixed entry point.

Let's try out debugging a local source file. Create `server.ts`:

```ts
import { serve } from "https://deno.land/std@$STD_VERSION/http/server.ts";
const server = serve({ port: 8000 });
console.log("http://localhost:8000/");

for await (const req of server) {
  req.respond({ body: "Hello World\n" });
}
```

Then we can set a breakpoint, and run the created configuration:

![VSCode debugger](../images/debugger7.jpg)

### JetBrains IDEs

You can debug Deno using your JetBrains IDE by right-clicking the file you want
to debug and selecting the `Debug 'Deno: <file name>'` option. This will create
a run/debug configuration with no permission flags set. To configure these flags
edit the run/debug configuration and modify the `Arguments` field with the
required flags.

### Other

Any client that implements the Devtools protocol should be able to connect to a
Deno process.

### Limitations

Devtools support is still immature. There is some functionality that is known to
be missing or buggy:

- autocomplete in Devtools' console causes the Deno process to exit.
- profiling and memory dumps might not work correctly.
