import {defineConfig,devices} from '@playwright/test';
export default defineConfig({
 testDir:'./tests/e2e',fullyParallel:false,workers:1,forbidOnly:!!process.env.CI,timeout:60000,expect:{timeout:15000},
 outputDir:'.runtime/playwright-results',
 webServer:{command:'node scripts/e2e-server.mjs',url:`${process.env.E2E_BASE_URL??'http://127.0.0.1:28088'}/api/v1/health/ready`,reuseExistingServer:false,timeout:300000,gracefulShutdown:{signal:'SIGTERM',timeout:15000}},
 reporter:[['list'],['html',{open:'never',outputFolder:'.runtime/playwright-report'}]],use:{baseURL:process.env.E2E_BASE_URL??'http://127.0.0.1:28088',channel:process.platform==='darwin'?'chrome':undefined,trace:'retain-on-failure',screenshot:'only-on-failure'},
 projects:[{name:'desktop',use:{...devices['Desktop Chrome'],viewport:{width:1440,height:1000}}},{name:'mobile',use:{...devices['Desktop Chrome'],viewport:{width:390,height:844},isMobile:true,hasTouch:true}}],
});
