console.log("sync 1");
setTimeout(() => {
  console.log("setTimeout 1");
  Promise.resolve().then(() => {
    console.log("Promise resolve in setTimeout 1");
  });
});
Promise.resolve().then(() => {
  console.log("promise 1");
});
window.close();
console.log("sync 2");
setTimeout(() => {
  console.log("setTimeout 2");
});
setTimeout(() => {
  console.log("setTimeout 3");
}, 100);
