async function main() {
  let socket = Deno.listen({
    address: "/tmp/rust-uds.sock",
    transport: "unix"
  });
  //   let socket = Deno.listen({
  // 	port: 8080,
  //     transport: "tcp"
  //   });

  let con = await socket.accept();

  let p = new Uint8Array(1024);
  await con.read(p);
  let b: number[] = p.reduce((sum, x) => {
    sum.push(x);
    return sum;
  }, new Array(p.length));
  console.log(String.fromCharCode(...b));

  setTimeout(() => {
    socket.close();
  }, 1000);
}
main();
