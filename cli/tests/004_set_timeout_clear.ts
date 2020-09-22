const handle = setTimeout(() => {
  console.log("timeout finish");
}, 10000);
clearTimeout(handle);
console.log("finish");
