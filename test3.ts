setInterval(async () => {
  await new Promise((resolve) => setTimeout(resolve, Math.random() * 1000));
  Deno.hostname();
}, 500);

setInterval(async () => {
  await new Promise((resolve) => setTimeout(resolve, Math.random() * 1000));
  Deno.env.get("foo");
}, 1100);

setInterval(async () => {
  await new Promise((resolve) => setTimeout(resolve, Math.random() * 1000));
  Deno.readTextFile("test3.ts");
}, 900);

setInterval(async () => {
  await new Promise((resolve) => setTimeout(resolve, Math.random() * 1000));
  fetch("https://example.com");
  console.log("test2");
}, 1000);
