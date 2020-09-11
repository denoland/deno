import { assertEquals } from "../../std/testing/asserts.ts";

// export const value = 'Successful import'; export default value;
import data1 from "data:application/javascript;base64,ZXhwb3J0IGNvbnN0IHZhbHVlID0gJ1N1Y2Nlc3NmdWwgaW1wb3J0JzsgZXhwb3J0IGRlZmF1bHQgdmFsdWU7";

Deno.test("static base64 data url import", () => {
  assertEquals(data1, "Successful import");
});

Deno.test("dynamic base64 data url import", async () => {
  const data2 = await import(
    // export const leet = 1337
    "data:application/javascript;base64,ZXhwb3J0IGNvbnN0IGxlZXQgPSAxMzM3"
  );
  assertEquals(data2.leet, 1337);
});

Deno.test("dynamic percent-encoding data url import", async () => {
  const data3 = await import(
    // export const value = 42;
    "data:application/javascript,export%20const%20value%20%3D%2042%3B"
  );
  assertEquals(data3.value, 42);
});

Deno.test("dynamic base64 typescript data url import", async () => {
  const data2 = await import(
    // export const leet: number = 1337;
    "data:application/typescript;base64,ZXhwb3J0IGNvbnN0IGxlZXQ6IG51bWJlciA9IDEzMzc7"
  );
  assertEquals(data2.leet, 1337);
});

Deno.test("spawn worker with data url", async () => {
  let resolve, timeout;
  const promise = new Promise((res, rej) => {
    resolve = res;
    timeout = setTimeout(() => rej("Worker timed out"), 2000);
  });

  const worker = new Worker(
    "data:application/javascript," +
      encodeURIComponent("self.onmessage = () => self.postMessage('Worker');"),
    { type: "module" },
  );

  worker.onmessage = (m) => {
    if (m.data === "Worker") {
      resolve();
    }
  };

  worker.postMessage();

  await promise;

  clearTimeout(timeout);
  worker.terminate();
});
