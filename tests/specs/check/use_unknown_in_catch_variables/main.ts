try {
  throw new Error();
} catch (e) {
  console.log(e.message);
}
