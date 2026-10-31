import { readdirSync } from 'node:fs';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';
function modules(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap(entry => {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) return modules(path);
    return /\.(?:mjs|js)$/.test(entry.name) && !entry.name.endsWith('.test.mjs') ? [path] : [];
  });
}
const files = ['check.mjs', ...modules('server'), ...modules('web')].sort();
for (const file of files) {
  const result = spawnSync(process.execPath, ['--check', file], { stdio: 'inherit' });
  if (result.status !== 0) process.exit(result.status || 1);
}
console.log(`Syntax checks passed for ${files.length} files.`);
