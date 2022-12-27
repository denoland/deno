const listener = Deno.listen({ port: 3500 });
const conn = await listener.accept();
console.log("unrefered from listener");
await listener.accept();
// await conn.write(new Uint8Array([1, 2, 3]));
// await conn.write(new Uint8Array([4, 5, 6]));
// await new Promise((resolve) =>
//   setTimeout(() => {
//     conn.close();
//     resolve();
//   }, 2000)
// );
// console.log("connection closed");
