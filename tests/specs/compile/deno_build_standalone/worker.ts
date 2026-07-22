// ensure this is properly set in a worker
const value: boolean = Deno.build.standalone;
console.log(value);
self.close();
