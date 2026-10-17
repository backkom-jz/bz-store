import { readCloudTestConfig } from './cloud-test-config.mjs';

const BASE_URL = 'http://127.0.0.1:28088';
const TOOL_KEYS = ['PATH', 'HOME', 'USER', 'LOGNAME', 'TMPDIR', 'TEMP', 'TMP', 'LANG', 'LC_ALL', 'TZ', 'CARGO_HOME', 'RUSTUP_HOME', 'RUSTUP_TOOLCHAIN', 'CARGO_BUILD_JOBS', 'SSL_CERT_FILE', 'SSL_CERT_DIR', 'CI'];
const TEST_KEYS = ['DEVSTORE_TEST_ALLOWED_HOSTS', 'MYSQL_TEST_DATABASE_URL', 'REDIS_TEST_URL', 'S3_TEST_ENDPOINT', 'S3_TEST_ACCESS_KEY', 'S3_TEST_SECRET_KEY', 'S3_TEST_BUCKET', 'S3_TEST_REGION', 'S3_TEST_VIRTUAL_HOSTED', 'SMTP_TEST_HOST', 'SMTP_TEST_PORT', 'SMTP_TEST_TLS', 'SMTP_TEST_FROM'];

export function buildE2EProcessEnv(env, role = 'runner') {
  const config = readE2EConfig(env);
  const tools = Object.fromEntries(TOOL_KEYS.filter((key) => env[key] !== undefined).map((key) => [key, env[key]]));
  if (role === 'api') return { ...tools, ...config.runtimeEnv };
  if (role === 'cargo' || role === 'gateway') return tools;
  if (role === 'build' || role === 'web') {
    const keys = ['APP_ENV', 'NUXT_API_INTERNAL_URL', 'NITRO_HOST', 'NITRO_PORT', 'NUXT_TELEMETRY_DISABLED'];
    return { ...tools, ...Object.fromEntries(keys.map((key) => [key, config.runtimeEnv[key]])) };
  }
  if (role !== 'runner') throw new Error('invalid E2E process role');
  return {
    ...tools,
    ...Object.fromEntries(TEST_KEYS.map((key) => [key, env[key].trim()])),
    ...config.runtimeEnv,
  };
}

export function redisCliInvocation(value, parentEnv = {}) {
  const url = new URL(value);
  if (!['redis:', 'rediss:'].includes(url.protocol) || !/^\/\d+$/.test(url.pathname) || url.search || url.hash) throw new Error('invalid test Redis URL');
  const username = decodeURIComponent(url.username);
  const password = decodeURIComponent(url.password);
  url.username = ''; url.password = '';
  const tools = Object.fromEntries(['PATH', 'LANG', 'LC_ALL', 'SSL_CERT_FILE', 'SSL_CERT_DIR'].filter((key) => parentEnv[key] !== undefined).map((key) => [key, parentEnv[key]]));
  return { args: ['-u', url.toString(), ...(username ? ['--user', username] : []), 'FLUSHDB'], env: { ...tools, REDISCLI_AUTH: password } };
}

export function readE2EConfig(env) {
  const cloud = readCloudTestConfig(env);
  if (env.E2E_BASE_URL && env.E2E_BASE_URL !== BASE_URL) {
    throw new Error('E2E_BASE_URL is fixed to the isolated local gateway');
  }
  if (!cloud.mailboxUrl) throw new Error('TEST_MAILBOX_API_URL is required for E2E');
  return {
    baseUrl: BASE_URL,
    apiPort: 28080,
    webPort: 23000,
    gatewayPort: 28088,
    mailboxUrl: cloud.mailboxUrl.replace(/\/$/, ''),
    redisUrl: cloud.redisUrl,
    runtimeEnv: {
      ...cloud.runtimeEnv,
      APP_PORT: '28080',
      APP_HOST: '127.0.0.1',
      SITE_URL: BASE_URL,
      E2E_BASE_URL: BASE_URL,
      NUXT_API_INTERNAL_URL: 'http://127.0.0.1:28080',
      NITRO_HOST: '127.0.0.1',
      NITRO_PORT: '23000',
      TRUSTED_PROXIES: '127.0.0.1',
      NUXT_TELEMETRY_DISABLED: '1',
    },
    commands: {
      api: ['target/debug/devstore-server'],
      web: [process.execPath, ['apps/web/.output/server/index.mjs']],
      gateway: [process.execPath, ['scripts/e2e-gateway.mjs']],
    },
  };
}
