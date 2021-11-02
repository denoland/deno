const i = setInterval(() => {
  Deno.emit("http://localhost:4545/subdir/mt_text_typescript.t1.ts");
  clearInterval(i);
}, 1);
