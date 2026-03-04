Deno.test(async function poc() {
  const ia = new Int32Array(
    new SharedArrayBuffer(Int32Array.BYTES_PER_ELEMENT),
  );

  const p = Promise.allSettled(
    Array(2).fill(0).map((_, i) => {
      const name = `poc_worker#${i}`;
      const w = new Worker(
        new URL("testdata/poc_worker.ts", import.meta.url),
        {
          type: "module",
          name,
        },
      );
      return new Promise<void>((res, rej) => {
        w.onmessage = (ev) => {
          if (ev.data.ok) res(ev.data);
          else rej(ev.data);
        };
        setTimeout(w.postMessage.bind(w), undefined, ia);
      }).finally(w.terminate.bind(w));
    }),
  );

  console.log(await p);
});
