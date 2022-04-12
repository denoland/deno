console.log("hello from worker");

self.onmessage = (e) => {
    if (e.data != "hello") {
        throw new Error("wrong message");
    }

    self.postMessage({ pid: process.pid });
}
