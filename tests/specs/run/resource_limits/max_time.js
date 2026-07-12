// Stay alive (but mostly idle) past the `--max-time` deadline.
setTimeout(() => console.log("should never run"), 60_000);
