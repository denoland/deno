console.log(Deno.permissions.query);
console.log(Deno.consoleSize);
self.onmessage = () => {
  self.close();
};
