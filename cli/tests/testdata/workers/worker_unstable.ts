console.log(Deno.permissions.query);
console.log(Deno.setRaw);
self.onmessage = () => {
  self.close();
};
