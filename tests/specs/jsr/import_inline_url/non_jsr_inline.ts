await import(
  "data:application/javascript,await%20import(%22http://localhost:4545/welcome.ts%22)"
);

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
