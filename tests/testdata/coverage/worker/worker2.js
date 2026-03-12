self.onmessage = () => {
  postMessage("hello from worker");
};

function onTimeout() {
  console.log("hello again");
}
// This code will not run, because worker will be terminated before
// the timeout fires.
setTimeout(onTimeout, 5_000);
