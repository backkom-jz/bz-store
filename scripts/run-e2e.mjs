import { spawn } from 'node:child_process';
import { buildE2EProcessEnv } from './e2e-config.mjs';

try {
  const env = buildE2EProcessEnv(process.env);
  const child = spawn(process.execPath, ['node_modules/@playwright/test/cli.js', 'test', ...process.argv.slice(2)], { env, stdio: 'inherit' });
  process.on('SIGTERM', () => child.kill('SIGTERM'));
  process.on('SIGINT', () => child.kill('SIGINT'));
  child.on('error', () => { process.stderr.write('could not start Playwright\n'); process.exitCode = 1; });
  child.on('exit', (code, signal) => { process.exitCode = code ?? (signal ? 1 : 0); });
} catch (error) {
  process.stderr.write(`${error.message}\n`);
  process.exitCode = 2;
}
