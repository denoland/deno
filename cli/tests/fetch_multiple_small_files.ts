await Promise.all((() => {
  const reqs = [];
  for (let i = 0; i < 100; ++i) {
    reqs.push(fetch("http://localhost:4545/single-small-file"));
  }
  return reqs.map(async (req) => {
    const resp = await req;
    const buff = await resp.arrayBuffer();
    if (buff.byteLength === 1_000_000) {
      return;
    } else {
      throw new Error("Downloaded file size is not the same from original");
    }
  });
})());
Deno.exit(0);
