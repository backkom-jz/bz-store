import { defineConfig, devices } from '@playwright/test';
export default defineConfig({
  testDir: './tests/admin-ui', fullyParallel: false, workers: 1, timeout: 30000,
  outputDir: '.runtime/admin-ui-results',
  reporter: [['list'], ['html', { open: 'never', outputFolder: '.runtime/admin-ui-report' }]],
  use: { baseURL: 'http://127.0.0.1:13777', channel: process.platform === 'darwin' ? 'chrome' : undefined, screenshot: 'only-on-failure', trace: 'retain-on-failure' },
  webServer: { command: 'npm run dev --workspace @devstore/admin -- --port 13777 --strictPort', url: 'http://127.0.0.1:13777/admin/', reuseExistingServer: false, env: { APP_ENV: 'test' } },
  projects: [
    { name: 'desktop', use: { ...devices['Desktop Chrome'], viewport: { width: 1440, height: 1000 } } },
    { name: 'mobile', use: { ...devices['Desktop Chrome'], viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true } },
  ],
});
