const p = new Promise(res => {
  setTimeout(() => {
    res(42)
  }, 1500)
})

console.log('the meaning of live, the universe and everything is:');

console.log(await p);
