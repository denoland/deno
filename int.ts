let i = 0;
let intervalId = setInterval(() => {
  console.log("hello world");
  i++;

  if (i > 2) {
    clearInterval(intervalId);
  }
}, 1000);


