const myRandomNumbers = new Array(100).fill(0).map(() => Math.random());
debugger;

setTimeout(() => {
  const myOtherRandomNumbers = new Array(100).fill(0).map(() => Math.random());
  debugger;
}, 3000);
