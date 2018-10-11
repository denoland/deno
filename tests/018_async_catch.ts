async function fn() {
  throw new Error("message");
}
async function call() {
  try {
    console.log("before await fn()");
    await fn();
    console.log("after await fn()");
  } catch (error) {
    console.log("catch");
  }
  console.log("after try-catch");
}
call().catch(() => console.log("outer catch"));
