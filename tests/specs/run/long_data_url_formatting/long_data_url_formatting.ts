await import(
  'data:application/typescript,console.trace("foo"); const error = new Error("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"); console.log(error.stack); throw error;'
);
