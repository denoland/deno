# wasi

This module provides an implementation of the WebAssembly System Interface

## Supported Syscalls

### wasi_snapshot_preview1

- [x] args_get
- [x] args_sizes_get
- [x] environ_get
- [x] environ_sizes_get
- [x] clock_res_get
- [x] clock_time_get
- [ ] fd_advise
- [ ] fd_allocate
- [x] fd_close
- [x] fd_datasync
- [x] fd_fdstat_get
- [ ] fd_fdstat_set_flags
- [ ] fd_fdstat_set_rights
- [x] fd_filestat_get
- [x] fd_filestat_set_size
- [x] fd_filestat_set_times
- [x] fd_pread
- [x] fd_prestat_get
- [x] fd_prestat_dir_name
- [x] fd_pwrite
- [x] fd_read
- [x] fd_readdir
- [x] fd_renumber
- [x] fd_seek
- [x] fd_sync
- [x] fd_tell
- [x] fd_write
- [x] path_create_directory
- [x] path_filestat_get
- [x] path_filestat_set_times
- [x] path_link
- [x] path_open
- [x] path_readlink
- [x] path_remove_directory
- [x] path_rename
- [x] path_symlink
- [x] path_unlink_file
- [x] poll_oneoff
- [x] proc_exit
- [ ] proc_raise
- [ ] sched_yield
- [x] random_get
- [ ] sock_recv
- [ ] sock_send
- [ ] sock_shutdown

## Usage

```typescript
import Context from "https://deno.land/std/wasi/snapshot_preview1.ts";

const context = new Context({
  args: Deno.args,
  env: Deno.env,
});

const binary = await Deno.readFile("path/to/your/module.wasm");
const module = await WebAssembly.compile(binary);
const instance = await WebAssembly.instantiate(module, {
  wasi_snapshot_preview1: context.exports,
});

context.memory = context.exports.memory;

if (module.exports._start) {
  instance.exports._start();
} else if (module.exports._initialize) {
  instance.exports._initialize();
} else {
  throw new Error("No entry point found");
}
```

## Testing

The test suite for this module spawns rustc processes to compile various example
Rust programs. You must have wasm targets enabled:

```
rustup target add wasm32-wasi
rustup target add wasm32-unknown-unknown
```
