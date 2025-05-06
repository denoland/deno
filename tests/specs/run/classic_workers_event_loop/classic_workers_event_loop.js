new Worker(
  "data:application/javascript,setTimeout(() => {console.log('done'); self.close()}, 1000)",
  { type: "classic" },
);
