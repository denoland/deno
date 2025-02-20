class Resource {
  [Symbol.dispose]() {
    console.log("Disposed");
  }
}
class AsyncResource {
  async [Symbol.asyncDispose]() {
    await new Promise((resolve) => setTimeout(resolve, 10));
    console.log("Async disposed");
  }
}

{
  using resource = new Resource();
  console.log("A");
}
{
  await using resource = new AsyncResource();
  console.log("B");
}
console.log("C");
