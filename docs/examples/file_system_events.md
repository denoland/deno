# File system events

## Concepts

- Use [Deno.watchFs](https://doc.deno.land/builtin/stable#Deno.watchFs) to watch
  for file system events.
- Results may vary between operating systems.

## Example

To poll for file system events in the current directory:

```ts
/**
 * watcher.ts
 */
const watcher = Deno.watchFs(".");
for await (const event of watcher) {
  console.log(">>>> event", event);
  // Example event: { kind: "create", paths: [ "/home/alice/deno/foo.txt" ] }
}
```

Run with:

```shell
deno run --allow-read watcher.ts
```

Now try adding, removing and modifying files in the same directory as
`watcher.ts`.

Note that the exact ordering of the events can vary between operating systems.
This feature uses different syscalls depending on the platform:

- Linux: [inotify](https://man7.org/linux/man-pages/man7/inotify.7.html)
- macOS:
  [FSEvents](https://developer.apple.com/library/archive/documentation/Darwin/Conceptual/FSEvents_ProgGuide/Introduction/Introduction.html)
- Windows:
  [ReadDirectoryChangesW](https://docs.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-readdirectorychangesw)
