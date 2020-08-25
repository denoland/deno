## File system events

To poll for file system events:

```ts
const watcher = Deno.watchFs("/");
for await (const event of watcher) {
  console.log(">>>> event", event);
  // { kind: "create", paths: [ "/foo.txt" ] }
}
```

Note that the exact ordering of the events can vary between operating systems.
This feature uses different syscalls depending on the platform:

- Linux: inotify
- macOS: FSEvents
- Windows: ReadDirectoryChangesW
