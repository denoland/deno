const id = setInterval(function() {
  console.log("test")
}, 200);

setTimeout(function() {
  clearInterval(id)
}, 500)
