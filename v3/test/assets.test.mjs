import test from 'node:test';
import assert from 'node:assert/strict';
import { createApp } from '../server/app.mjs';
test('all imported web modules are served and internal files are not', async () => {
 const app = createApp();
 await new Promise(resolve => app.server.listen(0, '127.0.0.1', resolve));
 const base = `http://127.0.0.1:${app.server.address().port}`;
 try {
  for (const name of ['app.js','navigation.js','reading-context.js','model-ui.js','thread-ui.js']) {
   const response = await fetch(`${base}/${name}`);
   assert.equal(response.status, 200, name);
   assert.match(response.headers.get('content-type'), /javascript/);
  }
  for (const name of ['README.md','render.test.mjs','server/app.mjs']) {
   assert.equal((await fetch(`${base}/${name}`)).status, 404, name);
  }
 } finally { await app.close(); }
});
