const buf = Buffer.alloc(2048, 0x41);
process.send(buf);
process.exit(0);
