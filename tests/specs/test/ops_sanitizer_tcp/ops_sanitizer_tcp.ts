Deno.test(function testLeakTcpOps() {
  const listener1 = Deno.listen({ port: 0 });
  listener1.accept();
});
