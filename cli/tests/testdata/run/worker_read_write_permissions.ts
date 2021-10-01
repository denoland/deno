new Worker(
  new URL("worker_read_write_permissions_worker.ts", import.meta.url),
  {
    deno: {
      namespace: true,
      permissions: {
        read: ["./file.txt"],
        write: ["./file.txt"],
      },
    },
    type: "module",
  },
);
