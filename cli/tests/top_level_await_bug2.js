const sleep = (n) => new Promise((r) => setTimeout(r, n));

await sleep(100);

export default 1;
