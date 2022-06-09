let count = 0;

console.log("0");

window.addEventListener("beforeunload", () => {
  console.log("GOT EVENT");
  if (count === 0 || count === 1) {
    setTimeout(() => {
      console.log("3");
    }, 100);
  }

  count++;
});

console.log("1");

setTimeout(() => {
  console.log("2");
}, 100);
