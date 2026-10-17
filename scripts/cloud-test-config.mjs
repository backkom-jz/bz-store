import { pathToFileURL } from 'node:url';
import { isIP } from 'node:net';

function required(env, key) {
  const value = env[key]?.trim();
  if (!value) throw new Error(`missing required test setting: ${key}`);
  return value;
}

function parseUrl(value, key, protocols) {
  let url;
  try {
    url = new URL(value);
  } catch {
    throw new Error(`invalid test setting: ${key}`);
  }
  if (!protocols.includes(url.protocol) || !url.hostname) {
    throw new Error(`invalid test setting: ${key}`);
  }
  return url;
}

function normalizeHost(host) {
  return host.toLowerCase().replace(/^\[|\]$/g, '').replace(/\.$/, '');
}

function isLoopback(host) {
  const normalized = normalizeHost(host);
  if (normalized === 'localhost') return true;
  if (isIP(normalized) === 4) return Number(normalized.split('.')[0]) === 127;
  if (isIP(normalized) !== 6) return false;

  const ipv6 = normalized.toLowerCase();
  if (ipv6 === '::' || ipv6 === '::1') return true;
  const words = expandIpv6(ipv6);
  if (!words) return false;
  if (words.slice(0, 5).every((word) => word === 0) && words[5] === 0xffff) {
    return (words[6] >> 8) === 127;
  }
  return false;
}

function expandIpv6(address) {
  const split = address.split('::');
  if (split.length > 2) return null;
  const parseSide = (side) => side ? side.split(':').map((word) => Number.parseInt(word, 16)) : [];
  const left = parseSide(split[0]);
  const right = parseSide(split[1] ?? '');
  if (left.some(Number.isNaN) || right.some(Number.isNaN)) return null;
  if (split.length === 1) return left.length === 8 ? left : null;
  const missing = 8 - left.length - right.length;
  if (missing < 1) return null;
  return [...left, ...Array(missing).fill(0), ...right];
}

export function readCloudTestConfig(env) {
  if (env.DEVSTORE_TEST_ENV !== 'cloud') {
    throw new Error('DEVSTORE_TEST_ENV must be cloud');
  }
  if (env.APP_ENV !== 'test') throw new Error('APP_ENV must be test');

  const allowedHosts = new Set(
    required(env, 'DEVSTORE_TEST_ALLOWED_HOSTS')
      .split(',')
      .map((host) => normalizeHost(host.trim()))
      .filter(Boolean),
  );
  if (allowedHosts.size === 0) throw new Error('DEVSTORE_TEST_ALLOWED_HOSTS must list test service hosts');
  const assertAllowedHost = (host, key) => {
    if (!allowedHosts.has(normalizeHost(host))) {
      throw new Error(`${key} host is not in DEVSTORE_TEST_ALLOWED_HOSTS`);
    }
  };
  const assertRemoteHost = (host, key) => {
    assertAllowedHost(host, key);
    if (isLoopback(host)) throw new Error(`${key} must use an isolated cloud test host`);
  };

  const databaseUrl = required(env, 'MYSQL_TEST_DATABASE_URL');
  const database = parseUrl(databaseUrl, 'MYSQL_TEST_DATABASE_URL', ['mysql:']);
  if (!database.username || database.pathname !== '/bz_ztore_test' || database.hash) {
    throw new Error('MYSQL_TEST_DATABASE_URL must target the dedicated bz_ztore_test database');
  }
  if (database.searchParams.has('socket')) {
    throw new Error('MYSQL_TEST_DATABASE_URL must not override the validated host with a socket');
  }
  assertRemoteHost(database.hostname, 'MYSQL_TEST_DATABASE_URL');

  const redisUrl = required(env, 'REDIS_TEST_URL');
  const redis = parseUrl(redisUrl, 'REDIS_TEST_URL', ['redis:', 'rediss:']);
  if (!/^\/\d+$/.test(redis.pathname) || redis.search || redis.hash) {
    throw new Error('REDIS_TEST_URL must select a database and must not include query or fragment');
  }
  assertRemoteHost(redis.hostname, 'REDIS_TEST_URL');

  const s3Endpoint = required(env, 'S3_TEST_ENDPOINT');
  const endpoint = parseUrl(s3Endpoint, 'S3_TEST_ENDPOINT', ['http:', 'https:']);
  if (endpoint.protocol !== 'https:' || endpoint.username || endpoint.password || endpoint.search || endpoint.hash) {
    throw new Error('S3_TEST_ENDPOINT must use HTTPS and must not include credentials, query, or fragment');
  }
  assertRemoteHost(endpoint.hostname, 'S3_TEST_ENDPOINT');
  const s3AccessKey = required(env, 'S3_TEST_ACCESS_KEY');
  const s3SecretKey = required(env, 'S3_TEST_SECRET_KEY');
  const s3Bucket = required(env, 'S3_TEST_BUCKET');
  if (s3Bucket !== 'devstore-test') throw new Error('S3_TEST_BUCKET must be devstore-test');
  const s3Region = required(env, 'S3_TEST_REGION');
  const s3VirtualHosted = required(env, 'S3_TEST_VIRTUAL_HOSTED');
  if (!['true', 'false'].includes(s3VirtualHosted)) {
    throw new Error('S3_TEST_VIRTUAL_HOSTED must be true or false');
  }

  const smtpHost = required(env, 'SMTP_TEST_HOST');
  const smtpPort = required(env, 'SMTP_TEST_PORT');
  if (!/^\d+$/.test(smtpPort) || Number(smtpPort) < 1 || Number(smtpPort) > 65535) {
    throw new Error('SMTP_TEST_PORT must be between 1 and 65535');
  }
  const smtpFrom = required(env, 'SMTP_TEST_FROM');
  const smtpTls = required(env, 'SMTP_TEST_TLS');
  if (!['true', 'false'].includes(smtpTls)) throw new Error('SMTP_TEST_TLS must be true or false');
  assertAllowedHost(smtpHost, 'SMTP_TEST_HOST');
  if (smtpTls === 'false' && !['localhost', '127.0.0.1', 'mailpit'].includes(normalizeHost(smtpHost))) {
    throw new Error('remote SMTP_TEST_HOST must use TLS');
  }
  const mailboxUrl = required(env, 'TEST_MAILBOX_API_URL');
  const mailbox = parseUrl(mailboxUrl, 'TEST_MAILBOX_API_URL', ['http:', 'https:']);
  if (mailbox.username || mailbox.password || mailbox.search || mailbox.hash) {
    throw new Error('TEST_MAILBOX_API_URL must not include credentials, query, or fragment');
  }
  assertAllowedHost(mailbox.hostname, 'TEST_MAILBOX_API_URL');
  if (mailbox.protocol === 'http:' && !isLoopback(mailbox.hostname)) {
    throw new Error('remote TEST_MAILBOX_API_URL must use HTTPS');
  }

  const runtimeEnv = {
    APP_ENV: 'test',
    APP_PORT: '28080',
    DATABASE_URL: databaseUrl,
    REDIS_URL: redisUrl,
    S3_ENDPOINT: s3Endpoint,
    S3_ACCESS_KEY: s3AccessKey,
    S3_SECRET_KEY: s3SecretKey,
    S3_BUCKET: s3Bucket,
    S3_REGION: s3Region,
    S3_VIRTUAL_HOSTED: s3VirtualHosted,
    SITE_URL: 'http://127.0.0.1:28088',
    JWT_SECRET: 'devstore-cloud-test-jwt-secret-only-32-bytes',
    CSRF_SECRET: 'devstore-cloud-test-csrf-secret-only-32-bytes',
    EMAIL_FROM: smtpFrom,
    EMAIL_OUTBOX_ENCRYPTION_KEY: Buffer.from(Array(32).fill(17)).toString('base64'),
    MFA_ENCRYPTION_KEY: Buffer.from(Array(32).fill(23)).toString('base64'),
    SMTP_HOST: smtpHost,
    SMTP_PORT: String(Number(smtpPort)),
    SMTP_USERNAME: '',
    SMTP_PASSWORD: '',
    SMTP_TLS: smtpTls,
    SMTP_TEST_HOST: smtpHost,
    SMTP_TEST_PORT: String(Number(smtpPort)),
    SMTP_TEST_FROM: smtpFrom,
    TEST_MAILBOX_API_URL: mailboxUrl,
    E2E_BASE_URL: 'http://127.0.0.1:28088',
    DEVSTORE_TEST_ENV: 'cloud',
  };

  return {
    databaseUrl,
    redisUrl,
    s3Endpoint,
    s3Bucket,
    mailboxUrl,
    allowedHosts,
    runtimeEnv,
  };
}

function quoteShell(value) {
  return `'${String(value).replaceAll("'", "'\\''")}'`;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    const config = readCloudTestConfig(process.env);
    if (process.argv[2] === '--shell') {
      for (const [key, value] of Object.entries(config.runtimeEnv)) {
        process.stdout.write(`export ${key}=${quoteShell(value)}\n`);
      }
    } else {
      process.stdout.write('cloud test configuration valid\n');
    }
  } catch (error) {
    process.stderr.write(`${error.message}\n`);
    process.exitCode = 2;
  }
}
