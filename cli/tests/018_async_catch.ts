function fn(): Promise<never> {
  throw new Error("message");
}
async function call(): Promise<void> {
  try {
    console.log("before await fn()");
    await fn();
    console.log("after await fn()");
  } catch (error) {
    console.log("catch");
  }
  console.log("after try-catch");
}
call().catch((): void => console.log("outer catch"));
