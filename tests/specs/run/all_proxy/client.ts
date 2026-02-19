async function test(url) {
  const res = await fetch(url);
  console.log(res);
  console.log(await res.text());
}

await test("http://insecure.invalid");
await test("https://secure.invalid");
