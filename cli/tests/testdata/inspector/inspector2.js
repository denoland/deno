console.log("hello from the script");

// This process will be killed before the timeout is over.
await new Promise((res, _) => setTimeout(res, 1000));
