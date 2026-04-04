const net = require('net');
const path = require('path');
const os = require('os');
const { Pipe, constants: PipeConstants } = require('internal/test/binding').internalBinding('pipe_wrap');

const tmpdir = path.join(os.tmpdir(), 'deno-pipe-test-' + process.pid);
require('fs').mkdirSync(tmpdir, { recursive: true });

const serverPath = path.join(tmpdir, 'server.sock');
const clientPath = path.join(tmpdir, 'client.sock');

// Clean up old sockets
try { require('fs').unlinkSync(serverPath); } catch {}
try { require('fs').unlinkSync(clientPath); } catch {}

console.error('Creating server...');
const server = net.createServer((socket) => {
  console.error('Server: got connection, handle type:', socket._handle?.constructor?.name);
  socket.on('data', (data) => {
    console.error('Server: received data:', data.toString());
    console.error('Server: calling socket.end()');
    socket.end('bye');
  });
  socket.on('end', () => {
    console.error('Server: client ended');
  });
  socket.on('close', () => {
    console.error('Server: socket closed');
    server.close();
  });
  socket.on('finish', () => {
    console.error('Server: socket finished (write side done)');
  });
  socket.on('error', (e) => {
    console.error('Server: socket error:', e.message, e.code);
  });
});

server.listen({ path: serverPath }, () => {
  console.error('Server listening at', serverPath);

  // Create a Pipe handle and bind it
  const handle = new Pipe(PipeConstants.SOCKET);
  const err = handle.bind(clientPath);
  console.error('Pipe bind result:', err, 'fd:', handle.fd);

  if (err < 0) {
    console.error('bind failed!');
    process.exit(1);
  }

  // Create socket with fd option
  const socket = new net.Socket({ fd: handle.fd, readable: true, writable: true });
  console.error('Socket created with fd:', handle.fd);

  socket.on('error', (e) => {
    console.error('Client: error:', e.message, e.code);
  });

  socket.on('connect', () => {
    console.error('Client: connected!');
    socket.write('hello');
  });

  socket.on('data', (data) => {
    console.error('Client: received data:', data.toString());
  });

  socket.on('end', () => {
    console.error('Client: got end');
  });

  socket.on('finish', () => {
    console.error('Client: finished (write side done)');
  });

  socket.on('close', () => {
    console.error('Client: socket closed');
    handle.close();
    // Clean up
    try { require('fs').unlinkSync(serverPath); } catch {}
    try { require('fs').unlinkSync(clientPath); } catch {}
    try { require('fs').rmdirSync(tmpdir); } catch {}
  });

  console.error('Connecting to', serverPath);
  socket.connect({ path: serverPath });
});

setTimeout(() => {
  console.error('TIMEOUT - test hung');
  process.exit(1);
}, 10000);
