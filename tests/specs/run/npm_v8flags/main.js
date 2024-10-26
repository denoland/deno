import v8flags from "npm:v8flags@4.0.1";

const flags = await new Promise((resolve, reject) => {
  v8flags((err, flags) => {
    if (err) {
      reject(err);
    } else {
      resolve(flags);
    }
  });
});

if (flags.length < 100) {
  throw new Error("Expected at least 100 flags");
}

console.log("ok");
