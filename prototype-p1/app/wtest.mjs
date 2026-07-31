const w = new Worker(new URL("./w2.mjs", import.meta.url), { type: "module" });
w.onmessage = (e) => { console.log("worker says:", e.data); w.terminate(); };
