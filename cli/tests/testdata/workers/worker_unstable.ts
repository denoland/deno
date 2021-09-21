console.log(Deno.permissions.query);
console.log(Deno.resolveDns);
self.onmessage = () => {
  self.close();
};
