console.log(Deno.permissions.query);
console.log(Deno.compile);
self.onmessage = () => {
  self.close();
};
