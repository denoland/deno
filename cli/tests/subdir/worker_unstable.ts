console.log(Deno.permissions.query);
console.log(Deno.emit);
self.onmessage = () => {
  self.close();
};
