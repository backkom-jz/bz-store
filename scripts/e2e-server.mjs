import { spawn, execFileSync } from 'node:child_process';
import { mkdirSync, lstatSync, chmodSync, writeFileSync, openSync, closeSync, unlinkSync } from 'node:fs';
import { createHmac, randomUUID } from 'node:crypto';
import { pathToFileURL } from 'node:url';
import { readE2EConfig, buildE2EProcessEnv } from './e2e-config.mjs';

export async function stopChildren(children, signal = 'SIGTERM', timeoutMs = 5000) {
  await Promise.all(children.map(async (child) => {
    if (child.exitCode !== null || child.signalCode !== null) return;
    if (child.__startupError && !child.pid) return;
    const exited = new Promise((resolve) => { child.once('exit', resolve); child.once('close', resolve); });
    child.kill(signal);
    const timedOut = Symbol('timed out');
    let timer;
    const result = await Promise.race([exited, new Promise((resolve) => { timer = setTimeout(() => resolve(timedOut), timeoutMs); })]);
    clearTimeout(timer);
    if (result === timedOut) {
      child.kill('SIGKILL');
      let finalTimer;
      const final = await Promise.race([exited, new Promise((resolve) => { finalTimer = setTimeout(() => resolve(timedOut), 1000); })]);
      clearTimeout(finalTimer);
      if (final === timedOut) throw new Error('E2E child did not exit after forced shutdown');
    }
  }));
}

function totp(secret, counter) {
  const alphabet = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ234567'; let bits = 0; let acc = 0; const bytes = [];
  for (const char of secret) { acc = (acc << 5) | alphabet.indexOf(char); bits += 5; if (bits >= 8) { bits -= 8; bytes.push((acc >> bits) & 255); } }
  const data = Buffer.alloc(8); data.writeBigUInt64BE(BigInt(counter));
  const digest = createHmac('sha1', Buffer.from(bytes)).update(data).digest(); const offset = digest[19] & 15;
  return String((((digest[offset] & 127) << 24) | (digest[offset + 1] << 16) | (digest[offset + 2] << 8) | digest[offset + 3]) % 1000000).padStart(6, '0');
}

async function waitFor(url, children, fetchImpl) {
  for (let attempt = 0; attempt < 120; attempt++) {
    if (children.some((child) => child.exitCode !== null || child.__startupError)) throw new Error('E2E process exited before becoming ready');
    try { if ((await fetchImpl(url, { signal: AbortSignal.timeout(1500) })).ok) return; } catch { /* retry until timeout */ }
    await new Promise((resolve) => setTimeout(resolve, 1000));
  }
  throw new Error('E2E service did not become ready');
}

export async function runE2EServer({ env = process.env, root = process.cwd(), spawnProcess = spawn, exec = execFileSync, fetchImpl = fetch } = {}) {
  const config = readE2EConfig(env);
  const runtimeEnv = buildE2EProcessEnv(env, 'api');
  const runtimeDir = `${root}/.runtime`;
  mkdirSync(runtimeDir, { recursive: true, mode: 0o700 });
  if (lstatSync(runtimeDir).isSymbolicLink()) throw new Error('E2E runtime directory must not be a symbolic link');
  chmodSync(runtimeDir, 0o700);
  const credentialPath = `${runtimeDir}/e2e.json`;
  // Recreate instead of overwriting: mode only applies when a file is created.
  try { unlinkSync(credentialPath); } catch (error) { if (error.code !== 'ENOENT') throw error; }
  const children = [];
  let stopping;
  const start = (command, args, logName, role) => {
    const fd = openSync(`${runtimeDir}/${logName}`, 'w', 0o600);
    const child = spawnProcess(command, args, { cwd: root, env: buildE2EProcessEnv(env, role), stdio: ['ignore', fd, fd] });
    closeSync(fd);
    child.once('error', () => { child.__startupError = true; });
    children.push(child);
    return child;
  };
  const cleanup = async (signal = 'SIGTERM') => {
    if (!stopping) stopping = (async () => {
      try { await stopChildren(children, signal); } finally {
        try { unlinkSync(credentialPath); } catch (error) { if (error.code !== 'ENOENT') throw error; }
        process.removeListener('SIGTERM', onTerm); process.removeListener('SIGINT', onInt);
      }
    })();
    return stopping;
  };
  const onSignal = (signal) => { void cleanup(signal).finally(() => process.exit(0)); };
  const onTerm = () => onSignal('SIGTERM'); const onInt = () => onSignal('SIGINT');
  process.once('SIGTERM', onTerm); process.once('SIGINT', onInt);

  try {
    const buildEnv = buildE2EProcessEnv(env, 'build');
    exec('npm', ['run', 'build', '-w', 'apps/admin'], { cwd: root, env: buildEnv, stdio: 'inherit' });
    writeFileSync(`${runtimeDir}/empty.env`, '', { mode: 0o600 });
    exec('npm', ['run', 'build', '-w', 'apps/web', '--', '--dotenv', `${runtimeDir}/empty.env`], { cwd: root, env: buildEnv, stdio: 'inherit' });
    exec('cargo', ['build', '--workspace'], { cwd: root, env: buildE2EProcessEnv(env, 'cargo'), stdio: 'inherit' });
    const credentials = { email: `admin-${randomUUID()}@example.test`, password: `E2e-${randomUUID()}-Only!` };
    exec('target/debug/admin', ['create', '--email', credentials.email, '--password-stdin'], { cwd: root, env: runtimeEnv, input: credentials.password, stdio: ['pipe', 'pipe', 'inherit'] });
    const mfaOutput = exec('target/debug/admin', ['mfa-begin', '--admin-email', credentials.email], { cwd: root, env: runtimeEnv, encoding: 'utf8' });
    const totpSecret = mfaOutput.match(/Authenticator secret \(shown once\): ([A-Z2-7]+)/)?.[1];
    if (!totpSecret) throw new Error('could not initialize E2E admin authentication');
    const mfaCounter = Math.floor(Date.now() / 30000);
    exec('target/debug/admin', ['mfa-confirm', '--admin-email', credentials.email, '--code-stdin'], { cwd: root, env: runtimeEnv, input: totp(totpSecret, mfaCounter), stdio: ['pipe', 'inherit', 'inherit'] });
    writeFileSync(credentialPath, JSON.stringify({ ...credentials, totpSecret, mfaCounter }), { mode: 0o600 });
    writeFileSync(`${runtimeDir}/e2e-payment.json`, JSON.stringify({ channel: 'bank', payee: '浏览器测试 · 请勿转账', account: 'TEST-ONLY-NOT-A-REAL-ACCOUNT', instructions: '仅用于自动化验收，禁止转账真实资金。' }), { mode: 0o600 });
    exec('target/debug/admin', ['init-storage'], { cwd: root, env: runtimeEnv, stdio: 'inherit' });
    exec('target/debug/admin', ['seed-demo', '--admin-email', credentials.email], { cwd: root, env: runtimeEnv, stdio: 'inherit' });
    exec('target/debug/admin', ['payment-config', '--admin-email', credentials.email, '--file', '.runtime/e2e-payment.json'], { cwd: root, env: runtimeEnv, stdio: 'inherit' });

    start('target/debug/devstore-server', [], 'e2e-api.log', 'api');
    start(process.execPath, ['apps/web/.output/server/index.mjs'], 'e2e-web.log', 'web');
    start(process.execPath, ['scripts/e2e-gateway.mjs'], 'e2e-gateway.log', 'gateway');
    await waitFor('http://127.0.0.1:28080/api/v1/health/ready', children, fetchImpl);
    await waitFor('http://127.0.0.1:23000/', children, fetchImpl);
    await waitFor(config.baseUrl, children, fetchImpl);
    return { config, children, cleanup };
  } catch (error) {
    await cleanup();
    throw error;
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    const runner = await runE2EServer();
    process.stdout.write(`E2E services ready at ${runner.config.baseUrl}\n`);
    for (const child of runner.children) child.once('exit', () => { void runner.cleanup().finally(() => process.exit(1)); });
    setInterval(() => {}, 30_000);
  } catch (error) {
    process.stderr.write(`${error.message}\n`);
    process.exitCode = 1;
  }
}
