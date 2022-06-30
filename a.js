// Extracted from <https://github.com/denoland/deno/blob/main/cli/bench/deno_common.js>

function bench(name, n, f) {
  const t1 = performance.now();
  for (let i = 0; i < n; ++i) {
    f(i);
  }
  const t2 = performance.now();

  const dt = (t2 - t1) / 1e3;
  const freq = n / dt;
  const time = (t2 - t1) / n;

  const msg = [
    `${name}:     \t`,
    `n = ${n},          \t`,
    `dt = ${dt.toFixed(3)}s, \t`,
    `freq = ${freq.toFixed(3)}/s, \t`,
  ];

  if (time >= 1) {
    msg.push(`time = ${time.toFixed(3)}ms/op`);
  } else {
    msg.push(`time = ${(time * 1e6).toFixed(0)}ns/op`);
  }

  console.log(msg.join(""));
}

function b64Long() {
  const input = "helloworld".repeat(1e5);
  bench("b64Long", 100, () => {
    atob(btoa(input));
  });
}

function b64Short() {
  const input = "123";
  bench("b64Short", 1e6, () => {
    atob(btoa(input));
  });
}

b64Long();
b64Short();
