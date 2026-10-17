import test from 'node:test';
import assert from 'node:assert/strict';
import { readCloudTestConfig } from './cloud-test-config.mjs';

const validEnv = () => ({
  APP_ENV: 'test',
  DEVSTORE_TEST_ENV: 'cloud',
  DEVSTORE_TEST_ALLOWED_HOSTS: 'mysql.test.invalid,redis.test.invalid,oss-test.invalid,mail.test.invalid,mailbox.test.invalid',
  MYSQL_TEST_DATABASE_URL: 'mysql://test_user:do-not-print@mysql.test.invalid:3306/bz_ztore_test',
  REDIS_TEST_URL: 'redis://:redis-secret@redis.test.invalid:6379/4',
  S3_TEST_ENDPOINT: 'https://oss-test.invalid',
  S3_TEST_ACCESS_KEY: 'test-access-key',
  S3_TEST_SECRET_KEY: 'test-secret-key',
  S3_TEST_BUCKET: 'devstore-test',
  S3_TEST_REGION: 'oss-cn-shanghai',
  S3_TEST_VIRTUAL_HOSTED: 'true',
  SMTP_TEST_HOST: 'mail.test.invalid',
  SMTP_TEST_PORT: '1025',
  SMTP_TEST_TLS: 'true',
  SMTP_TEST_FROM: 'DevStore Test <test@example.test>',
  TEST_MAILBOX_API_URL: 'https://mailbox.test.invalid/api',
});

test('accepts a complete dedicated cloud test environment', () => {
  const config = readCloudTestConfig(validEnv());
  assert.equal(config.databaseUrl, validEnv().MYSQL_TEST_DATABASE_URL);
  assert.equal(config.runtimeEnv.APP_ENV, 'test');
  assert.equal(config.runtimeEnv.S3_BUCKET, 'devstore-test');
  assert.equal(config.runtimeEnv.SMTP_TLS, 'true');
});

test('rejects non-test database names and malformed database targets without echoing credentials', () => {
  for (const database of ['bz_ztore', 'erp_hzs', '', 'bz_ztore_test/']) {
    const env = validEnv();
    env.MYSQL_TEST_DATABASE_URL = `mysql://private-user:private-password@mysql.test.invalid:3306/${database}`;
    assert.throws(() => readCloudTestConfig(env));
  }
  const env = validEnv();
  env.MYSQL_TEST_DATABASE_URL = 'not-a-dsn-private-user-private-password';
  assert.throws(() => readCloudTestConfig(env), (error) => {
    assert.doesNotMatch(error.message, /private-user|private-password/);
    return true;
  });
});

test('rejects production or missing cloud-only configuration', () => {
  const mutations = [
    (env) => { env.APP_ENV = 'production'; },
    (env) => { env.DEVSTORE_TEST_ENV = ''; },
    (env) => { env.DEVSTORE_TEST_ALLOWED_HOSTS = ''; },
    (env) => { env.REDIS_TEST_URL = ''; },
    (env) => { env.S3_TEST_ENDPOINT = ''; },
    (env) => { env.S3_TEST_ACCESS_KEY = ''; },
    (env) => { env.S3_TEST_SECRET_KEY = ''; },
    (env) => { env.S3_TEST_BUCKET = 'production-assets'; },
    (env) => { env.S3_TEST_REGION = ''; },
    (env) => { env.S3_TEST_VIRTUAL_HOSTED = ''; },
    (env) => { env.SMTP_TEST_HOST = ''; },
    (env) => { env.SMTP_TEST_TLS = 'sometimes'; },
    (env) => { env.TEST_MAILBOX_API_URL = ''; },
  ];
  for (const mutate of mutations) {
    const env = validEnv();
    mutate(env);
    assert.throws(() => readCloudTestConfig(env));
  }
});

test('rejects MySQL socket overrides that bypass the validated remote host', () => {
  for (const query of ['socket=%2Ftmp%2Fprivate-mysql.sock', 'ssl-mode=VERIFY_IDENTITY&s%6fcket=%2Ftmp%2Fprivate-mysql.sock']) {
    const env = validEnv();
    env.MYSQL_TEST_DATABASE_URL += '?' + query;
    assert.throws(() => readCloudTestConfig(env), (error) => {
      assert.doesNotMatch(error.message, /do-not-print|private-mysql/);
      return true;
    });
  }
  const env = validEnv();
  env.MYSQL_TEST_DATABASE_URL += '?ssl-mode=VERIFY_IDENTITY';
  assert.equal(readCloudTestConfig(env).runtimeEnv.DATABASE_URL, env.MYSQL_TEST_DATABASE_URL);
});

test('rejects resource hosts outside the explicit test allowlist and local DB/Redis/S3 endpoints', () => {
  const cases = [
    (env) => { env.MYSQL_TEST_DATABASE_URL = 'mysql://test_user:secret@prod-db.invalid:3306/bz_ztore_test'; },
    (env) => { env.MYSQL_TEST_DATABASE_URL = 'mysql://test_user:secret@localhost:3306/bz_ztore_test'; env.DEVSTORE_TEST_ALLOWED_HOSTS += ',localhost'; },
    (env) => { env.REDIS_TEST_URL = 'redis://localhost:6379/4'; env.DEVSTORE_TEST_ALLOWED_HOSTS += ',localhost'; },
    (env) => { env.REDIS_TEST_URL = 'redis://:secret@redis.test.invalid:6379/4?password=also-secret'; },
    (env) => { env.REDIS_TEST_URL = 'redis://redis.test.invalid:6379/4#secret'; },
    (env) => { env.S3_TEST_ENDPOINT = 'http://oss-test.invalid'; },
    (env) => { env.S3_TEST_ENDPOINT = 'https://127.0.0.1:9000'; env.DEVSTORE_TEST_ALLOWED_HOSTS += ',127.0.0.1'; },
    (env) => { env.S3_TEST_ENDPOINT = 'https://prod-oss.invalid'; },
    (env) => { env.SMTP_TEST_HOST = 'smtp.production.invalid'; },
    (env) => { env.SMTP_TEST_HOST = '127.0.0.2'; env.SMTP_TEST_TLS = 'false'; env.DEVSTORE_TEST_ALLOWED_HOSTS += ',127.0.0.2'; },
    (env) => { env.TEST_MAILBOX_API_URL = 'https://mail.production.invalid/api'; },
    (env) => {
      env.MYSQL_TEST_DATABASE_URL = 'mysql://test_user:secret@[::ffff:7f00:1]:3306/bz_ztore_test';
      env.DEVSTORE_TEST_ALLOWED_HOSTS += ',::ffff:7f00:1';
    },
    (env) => {
      env.REDIS_TEST_URL = 'redis://[::]:6379/4';
      env.DEVSTORE_TEST_ALLOWED_HOSTS += ',::';
    },
    (env) => {
      env.TEST_MAILBOX_API_URL = 'http://mailbox.test.invalid/api';
    },
  ];
  for (const mutate of cases) {
    const env = validEnv();
    mutate(env);
    assert.throws(() => readCloudTestConfig(env));
  }
});
