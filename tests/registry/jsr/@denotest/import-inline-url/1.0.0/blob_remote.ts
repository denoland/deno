const url = URL.createObjectURL(
  new Blob(['await import("http://localhost:4545/welcome.ts")'], {
    type: "text/javascript",
  }),
);

try {
  await import(url);
} finally {
  URL.revokeObjectURL(url);
}
