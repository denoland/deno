(async (): Promise<void> => {
  try {
    Deno.readFileSync("./tests/043_worker_perm/a/a.txt");
  } catch {
    console.log("UNEXPECTED read failure for a.txt");
  }

  try {
    Deno.writeFileSync(
      "./tests/043_worker_perm/a/a.txt",
      new TextEncoder().encode("A\n")
    );
    console.log("UNEXPECTED write success for a.txt");
  } catch {}

  try {
    Deno.readFileSync("./tests/043_worker_perm/b/b.txt");
    console.log("UNEXPECTED read success for b.txt");
  } catch {}

  try {
    Deno.writeFileSync(
      "./tests/043_worker_perm/b/b.txt",
      new TextEncoder().encode("B\n")
    );
  } catch {
    console.log("UNEXPECTED write failure for b.txt");
  }

  try {
    const conn = await Deno.dial("tcp", "127.0.0.1:4545");
    conn.close();
  } catch {
    console.log("UNEXPECTED dial failure for 127.0.0.1:4545");
  }

  try {
    const conn = await Deno.dial("tcp", "127.0.0.1:4546");
    conn.close();
    console.log("UNEXPECTED dial success for 127.0.0.1:4546");
  } catch {}

  postMessage("DONE");
  workerClose();
})();
