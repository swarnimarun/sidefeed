import { readdirSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
const files = ['check.mjs', ...readdirSync('server').filter(x => x.endsWith('.mjs')).map(x => `server/${x}`), ...readdirSync('web').filter(x => x.endsWith('.js')).map(x => `web/${x}`)];
for (const file of files) {
  const result = spawnSync(process.execPath, ['--check', file], { stdio: 'inherit' });
  if (result.status !== 0) process.exit(result.status || 1);
}
console.log(`Syntax checks passed for ${files.length} files.`);
