new Worker("non_existent.js", {
  deno: {
    permissions: {
      read: ["./file.txt"],
      write: ["./file.txt"],
    },
  },
  type: "module",
});
