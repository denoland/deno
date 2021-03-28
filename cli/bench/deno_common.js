// Run with: deno run -A ./cli/bench/deno_common.js
function benchSync(name, n, innerLoop) {
  const t1 = Date.now();
  for (let i = 0; i < n; i++) {
    innerLoop(i);
  }
  const t2 = Date.now();
  const dt = (t2 - t1) / 1e3;
  const r = n / dt;
  const ns = Math.floor(dt / n * 1e9);
  console.log(
    `${name}:${" ".repeat(20 - name.length)}\t` +
      `n = ${n}, dt = ${dt.toFixed(3)}s, r = ${r.toFixed(0)}/s, t = ${ns}ns/op`,
  );
}

function benchUrlParse() {
  benchSync("url_parse", 5e4, (i) => {
    new URL(`http://www.google.com/${i}`);
  });
}

function benchNow() {
  benchSync("now", 5e5, () => {
    performance.now();
  });
}

function benchWriteNull() {
  // Not too large since we want to measure op-overhead not sys IO
  const dataChunk = new Uint8Array(100);
  const file = Deno.openSync("/dev/null", { write: true });
  benchSync("write_null", 5e5, () => {
    Deno.writeSync(file.rid, dataChunk);
  });
  Deno.close(file.rid);
}

function benchReadZero() {
  const buf = new Uint8Array(100);
  const file = Deno.openSync("/dev/zero");
  benchSync("read_zero", 5e5, () => {
    Deno.readSync(file.rid, buf);
  });
  Deno.close(file.rid);
}

function main() {
  benchUrlParse();
  benchNow();
  benchWriteNull();
  benchReadZero();
}
main();
