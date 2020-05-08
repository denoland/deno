## Internal details

### Deno and Linux analogy

|                       **Linux** | **Deno**                         |
| ------------------------------: | :------------------------------- |
|                       Processes | Web Workers                      |
|                        Syscalls | Ops                              |
|           File descriptors (fd) | [Resource ids (rid)](#resources) |
|                       Scheduler | Tokio                            |
| Userland: libc++ / glib / boost | https://deno.land/std/           |
|                 /proc/\$\$/stat | [Deno.metrics()](#metrics)       |
|                       man pages | deno types                       |

#### Resources

Resources (AKA `rid`) are Deno's version of file descriptors. They are integer
values used to refer to open files, sockets, and other concepts. For testing it
would be good to be able to query the system for how many open resources there
are.

```ts
const { resources, close } = Deno;
console.log(resources());
// { 0: "stdin", 1: "stdout", 2: "stderr" }
close(0);
console.log(resources());
// { 1: "stdout", 2: "stderr" }
```

#### Metrics

Metrics is Deno's internal counter for various statistics.

```shell
> console.table(Deno.metrics())
┌──────────────────┬────────┐
│     (index)      │ Values │
├──────────────────┼────────┤
│  opsDispatched   │   9    │
│   opsCompleted   │   9    │
│ bytesSentControl │  504   │
│  bytesSentData   │   0    │
│  bytesReceived   │  856   │
└──────────────────┴────────┘
```

### Schematic diagram

![architectural schematic](https://deno.land/images/schematic_v0.2.png)
