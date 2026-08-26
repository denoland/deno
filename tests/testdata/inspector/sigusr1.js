// Keep the event loop alive so the inspector can be activated via SIGUSR1.
setInterval(() => {}, 1000);
// The SIGUSR1 listener is only installed after a 500ms grace period; print
// the marker well after that so the test knows it is safe to send the signal.
setTimeout(() => console.log("ready"), 1000);
