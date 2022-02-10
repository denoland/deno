console.log("hello from worker");
// self.postMessage("hello from worker");

self.onmessage = (e) => {
    console.log("onmessage handler called");
    if (e.data != "hello") {
        throw new Error("wrong message");
    }

    self.postMessage({ pid: process.pid });
}
