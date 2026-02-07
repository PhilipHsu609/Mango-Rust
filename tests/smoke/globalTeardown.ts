import { stopServer } from '../helpers/server.js';

async function globalTeardown() {
  console.log('Smoke tests: Stopping server...');
  await stopServer();
  console.log('Smoke tests: Server stopped');
}

export default globalTeardown;
