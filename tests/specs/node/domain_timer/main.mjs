import domain from "node:domain";
import EventEmitter from "node:events";

// Test 1: process.domain is preserved in setTimeout
await new Promise((resolve) => {
  const d = domain.create();
  d.on("error", () => {});
  d.run(() => {
    setTimeout(() => {
      if (process.domain === d) {
        console.log("ok 1 - process.domain preserved in setTimeout");
      } else {
        console.log("not ok 1 - process.domain is", process.domain);
      }
      resolve();
    }, 10);
  });
});

// Test 2: domain catches errors thrown in setTimeout
await new Promise((resolve) => {
  const d = domain.create();
  d.on("error", (err) => {
    if (err.message === "timer error") {
      console.log("ok 2 - domain catches errors in setTimeout");
    } else {
      console.log("not ok 2 - wrong error:", err.message);
    }
    resolve();
  });
  d.run(() => {
    setTimeout(() => {
      throw new Error("timer error");
    }, 10);
  });
});

// Test 3: domain catches errors emitted on EventEmitter in setTimeout
await new Promise((resolve) => {
  const d = domain.create();
  const ee = new EventEmitter();
  d.on("error", (err) => {
    if (err.message === "ee error") {
      console.log("ok 3 - domain catches EE errors in setTimeout");
    } else {
      console.log("not ok 3 - wrong error:", err.message);
    }
    resolve();
  });
  d.run(() => {
    setTimeout(() => {
      ee.emit("error", new Error("ee error"));
    }, 10);
  });
});

// Test 4: process.domain preserved in setInterval
await new Promise((resolve) => {
  const d = domain.create();
  d.on("error", () => {});
  d.run(() => {
    const id = setInterval(() => {
      clearInterval(id);
      if (process.domain === d) {
        console.log("ok 4 - process.domain preserved in setInterval");
      } else {
        console.log("not ok 4 - process.domain is", process.domain);
      }
      resolve();
    }, 10);
  });
});

// Test 5: nested setTimeout preserves domain
await new Promise((resolve) => {
  const d = domain.create();
  d.on("error", () => {});
  d.run(() => {
    setTimeout(() => {
      setTimeout(() => {
        if (process.domain === d) {
          console.log("ok 5 - nested setTimeout preserves domain");
        } else {
          console.log("not ok 5 - process.domain is", process.domain);
        }
        resolve();
      }, 10);
    }, 10);
  });
});
