const id = setInterval(() => {
  console.log("test");
}, 200);

setTimeout(() => {
  clearInterval(id);
}, 500);
