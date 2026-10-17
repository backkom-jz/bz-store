import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile, rm, access, stat, chmod, symlink } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { request as httpRequest } from 'node:http';
import { readE2EConfig, buildE2EProcessEnv, redisCliInvocation } from './e2e-config.mjs';
import { createE2EGateway } from './e2e-gateway.mjs';
import { runE2EServer, stopChildren } from './e2e-server.mjs';

const validEnv = () => ({
  APP_ENV: 'test', DEVSTORE_TEST_ENV: 'cloud',
  DEVSTORE_TEST_ALLOWED_HOSTS: 'mysql.test.invalid,redis.test.invalid,oss-test.invalid,mail.test.invalid,mailbox.test.invalid',
  MYSQL_TEST_DATABASE_URL: 'mysql://user:private-password@mysql.test.invalid:3306/bz_ztore_test',
  REDIS_TEST_URL: 'redis://:redis-private-password@redis.test.invalid:6379/4',
  S3_TEST_ENDPOINT: 'https://oss-test.invalid', S3_TEST_ACCESS_KEY: 'private-access-key',
  S3_TEST_SECRET_KEY: 'private-secret-key', S3_TEST_BUCKET: 'devstore-test',
  S3_TEST_REGION: 'oss-cn-shanghai', S3_TEST_VIRTUAL_HOSTED: 'true',
  SMTP_TEST_HOST: 'mail.test.invalid', SMTP_TEST_PORT: '1025', SMTP_TEST_TLS: 'true',
  SMTP_TEST_FROM: 'DevStore Test <test@example.test>',
  TEST_MAILBOX_API_URL: 'https://mailbox.test.invalid/api',
});

test('E2E config requires cloud test identity and mailbox and pins local-only ports', () => {
  const env = validEnv();
  const config = readE2EConfig(env);
  assert.equal(config.baseUrl, 'http://127.0.0.1:28088');
  assert.equal(config.apiPort, 28080);
  assert.equal(config.webPort, 23000);
  assert.equal(config.gatewayPort, 28088);
  assert.equal(config.runtimeEnv.DATABASE_URL, env.MYSQL_TEST_DATABASE_URL);
  assert.equal(config.runtimeEnv.APP_HOST, '127.0.0.1');
  assert.equal(config.runtimeEnv.NITRO_HOST, '127.0.0.1');
  for (const key of ['MYSQL_TEST_DATABASE_URL', 'REDIS_TEST_URL', 'S3_TEST_SECRET_KEY']) {
    assert.ok(!JSON.stringify(config.commands).includes(env[key]));
  }
  for (const broken of [
    { ...env, DEVSTORE_TEST_ENV: '' },
    { ...env, TEST_MAILBOX_API_URL: '' },
    { ...env, E2E_BASE_URL: 'https://store.bengzi.cn' },
  ]) assert.throws(() => readE2EConfig(broken));
});

test('gateway serves admin SPA fallback and rejects traversal', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'bz-store-admin-'));
  await mkdir(join(directory, 'assets'));
  await writeFile(join(directory, 'index.html'), '<main>admin-app</main>');
  await writeFile(join(directory, 'assets', 'app.js'), 'safe');
  const gateway = createE2EGateway({ adminDir: directory, apiPort: 0, webPort: 0 });
  await new Promise((resolve) => gateway.listen(0, '127.0.0.1', resolve));
  const address = gateway.address();
  const origin = `http://127.0.0.1:${address.port}`;
  try {
    assert.match(await (await fetch(`${origin}/admin/tutorials/one`)).text(), /admin-app/);
    assert.equal((await fetch(`${origin}/admin/assets/app.js`)).status, 200);
    const traversalStatus = await new Promise((resolve, reject) => {
      const request = httpRequest({ hostname: '127.0.0.1', port: address.port, path: '/admin/%2e%2e/package.json' }, (response) => { response.resume(); resolve(response.statusCode); });
      request.on('error', reject); request.end();
    });
    assert.equal(traversalStatus, 400);
  } finally {
    await new Promise((resolve, reject) => gateway.close((error) => error ? reject(error) : resolve()));
    await rm(directory, { recursive: true, force: true });
  }
});

test('only approved toolchain and validated test settings reach child processes', () => {
  const env = buildE2EProcessEnv({ ...validEnv(), PATH: '/tool/bin', AWS_SECRET_ACCESS_KEY: 'production-secret', NODE_OPTIONS: '--inspect', OTHER_API_TOKEN: 'production-token' });
  assert.equal(env.PATH, '/tool/bin');
  assert.equal(env.APP_HOST, '127.0.0.1');
  assert.equal(env.MYSQL_TEST_DATABASE_URL, validEnv().MYSQL_TEST_DATABASE_URL);
  for (const key of ['AWS_SECRET_ACCESS_KEY', 'NODE_OPTIONS', 'OTHER_API_TOKEN']) assert.equal(env[key], undefined);
  for (const role of ['build', 'cargo', 'web', 'gateway']) {
    const child = buildE2EProcessEnv(validEnv(), role);
    for (const key of ['DATABASE_URL', 'MYSQL_TEST_DATABASE_URL', 'REDIS_URL', 'S3_SECRET_KEY', 'JWT_SECRET']) assert.equal(child[key], undefined, `${role} must not receive ${key}`);
  }
});

test('Redis ACL username is retained while password is passed only in environment', () => {
  const invocation = redisCliInvocation('rediss://test-user:private%40password@redis.test.invalid:6380/4');
  assert.deepEqual(invocation.args, ['-u', 'rediss://redis.test.invalid:6380/4', '--user', 'test-user', 'FLUSHDB']);
  assert.equal(invocation.env.REDISCLI_AUTH, 'private@password');
  assert.doesNotMatch(invocation.args.join(' '), /private|password/);
  const isolated = redisCliInvocation('redis://:secret@redis.test.invalid:6379/4', {
    PATH: '/tool/bin', LANG: 'C.UTF-8', DATABASE_URL: 'private-db', S3_SECRET_KEY: 'private-oss', REDISCLI_AUTH: 'stale-password',
  });
  assert.deepEqual(isolated.env, { PATH: '/tool/bin', LANG: 'C.UTF-8', REDISCLI_AUTH: 'secret' });
});

test('spawn failure with close and no exit cannot hang cleanup', { timeout: 200 }, async () => {
  const { EventEmitter } = await import('node:events');
  const child = Object.assign(new EventEmitter(), { exitCode: null, signalCode: null, kill: () => { setImmediate(() => child.emit('close', -2)); return false; } });
  await stopChildren([child], 'SIGTERM', 10);
});

for (const signal of ['SIGTERM', 'SIGINT']) test(`runner stops and awaits only owned children on ${signal}`, async () => {
  const events = [];
  const { EventEmitter } = await import('node:events');
  const children = [1, 2].map((id) => {
    const emitter = new EventEmitter();
    return {
      kill: (sent) => { events.push(`kill-${id}-${sent}`); setTimeout(() => emitter.emit('exit', 0), 1); },
      once: emitter.once.bind(emitter), on: emitter.on.bind(emitter),
      exitCode: null, signalCode: null,
    };
  });
  await stopChildren(children, signal);
  assert.deepEqual(events, [`kill-1-${signal}`, `kill-2-${signal}`]);
});

test('failed API startup cleans runner-owned children and generated credentials', async () => {
  const root = await mkdtemp(join(tmpdir(), 'bz-store-e2e-runner-'));
  await mkdir(join(root, 'apps/admin/dist'), { recursive: true });
  const calls = [];
  const { EventEmitter } = await import('node:events');
  const spawnProcess = (command, args) => {
    calls.push([command, ...args]);
    const emitter = new EventEmitter();
    const failed = calls.filter(([name]) => name !== 'kill-owned').length === 2;
    const child = Object.assign(emitter, { exitCode: failed ? 1 : null, signalCode: null, kill: () => { calls.push(['kill-owned']); child.exitCode = 0; setImmediate(() => emitter.emit('exit', 0)); } });
    return child;
  };
  try {
    await assert.rejects(runE2EServer({
      env: validEnv(), root, spawnProcess,
      exec: (_command, args) => args.includes('mfa-begin') ? 'Authenticator secret (shown once): JBSWY3DPEHPK3PXP\n' : '',
      fetchImpl: async () => { throw new Error('not ready'); },
    }), /E2E process exited/);
    await assert.rejects(access(join(root, '.runtime/e2e.json')));
    assert.equal(calls.filter(([name]) => name === 'kill-owned').length, 2);
    assert.ok(!calls.flat().join(' ').includes('docker'));
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test('runner replaces stale readable credentials and secures existing runtime directory', async () => {
  const root = await mkdtemp(join(tmpdir(), 'bz-store-e2e-permissions-'));
  const runtime = join(root, '.runtime');
  const credential = join(runtime, 'e2e.json');
  await mkdir(runtime);
  await chmod(runtime, 0o755);
  await writeFile(credential, 'stale', { mode: 0o644 });
  const { EventEmitter } = await import('node:events');
  const spawnProcess = () => {
    const child = Object.assign(new EventEmitter(), { exitCode: null, signalCode: null, kill: () => {
      child.exitCode = 0; setImmediate(() => child.emit('exit', 0));
    } });
    return child;
  };
  let runner;
  try {
    runner = await runE2EServer({ env: validEnv(), root, spawnProcess,
      exec: (_command, args) => args.includes('mfa-begin') ? 'Authenticator secret (shown once): JBSWY3DPEHPK3PXP\n' : '',
      fetchImpl: async () => ({ ok: true }),
    });
    assert.equal((await stat(credential)).mode & 0o777, 0o600);
    assert.equal((await stat(runtime)).mode & 0o777, 0o700);
  } finally {
    await runner?.cleanup();
    await rm(root, { recursive: true, force: true });
  }
});

test('runner rejects a runtime directory symlink without modifying its target', async () => {
  const root = await mkdtemp(join(tmpdir(), 'bz-store-e2e-symlink-'));
  const target = join(root, 'shared');
  await mkdir(target);
  await chmod(target, 0o755);
  await symlink(target, join(root, '.runtime'));
  try {
    await assert.rejects(runE2EServer({ env: validEnv(), root, exec: () => {
      throw new Error('must reject runtime path before running commands');
    } }), /runtime directory must not be a symbolic link/);
    assert.equal((await stat(target)).mode & 0o777, 0o755);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test('persists consumed administrator counters between browser project workers', async () => {
  const { claimAdminCounter } = await import('./e2e-admin-state.mjs');
  const { readFile } = await import('node:fs/promises');
  const directory = await mkdtemp(join(tmpdir(), 'devstore-e2e-auth-'));
  const file = join(directory, 'e2e.json');
  const credentials = { email: 'fixture@example.test', password: 'fixture-only', totpSecret: 'TESTSEED', mfaCounter: 100 };
  try {
    await writeFile(file, JSON.stringify(credentials), { mode: 0o600 });
    assert.equal(claimAdminCounter(file, 100), false);
    assert.equal(claimAdminCounter(file, 101), true);
    assert.equal(JSON.parse(await readFile(file, 'utf8')).mfaCounter, 101);
    assert.equal(claimAdminCounter(file, 101), false);
    assert.equal(claimAdminCounter(file, 102), true);
    assert.equal(JSON.parse(await readFile(file, 'utf8')).password, credentials.password);
    assert.equal((await stat(file)).mode & 0o777, 0o600);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});
