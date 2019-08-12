const w = new Worker("./043_worker_perm/worker.ts", {
  allowRead: ["./tests/043_worker_perm/a/"],
  allowWrite: ["./tests/043_worker_perm/b"],
  allowNet: ["127.0.0.1:4545"]
});

const w2 = new Worker("./043_worker_perm/worker_2.ts", {
  allowRead: ["*", "./tests/043_worker_perm/a/"],
  allowWrite: ["*"]
});

let count = 0;

const handler = (): void => {
  count++;
  if (count === 2) {
    console.log("DONE");
    Deno.exit(0);
  }
};

w.onmessage = handler;
w2.onmessage = handler;
