import {spawn,execFileSync} from 'node:child_process';
import {mkdirSync,writeFileSync,openSync} from 'node:fs';
import {randomUUID} from 'node:crypto';
const root=process.cwd();
mkdirSync('.runtime',{recursive:true});mkdirSync('apps/admin/dist',{recursive:true});
const compose=['compose','-f','deploy/docker-compose.test.yml'];
execFileSync('docker',[...compose,'up','-d','--wait','--wait-timeout','120','postgres','redis','storage','clamav'],{stdio:'inherit'});
const schema='e2e_'+randomUUID().replaceAll('-','');
execFileSync('docker',[...compose,'exec','-T','postgres','psql','-U','devstore_test','-d','devstore_test','-v','ON_ERROR_STOP=1','-c','CREATE SCHEMA '+schema],{stdio:'pipe'});
const database=new URL('postgres://devstore_test:devstore_test_only@127.0.0.1:15432/devstore_test');
database.searchParams.set('options','-c search_path='+schema);
const env={...process.env,APP_ENV:'test',APP_PORT:'18080',DATABASE_URL:database.href,REDIS_URL:'redis://127.0.0.1:16379',S3_ENDPOINT:'http://127.0.0.1:19000',S3_ACCESS_KEY:'devstore_test',S3_SECRET_KEY:'devstore_test_only',S3_BUCKET:'devstore-test',CLAMAV_ADDR:'127.0.0.1:13310',SITE_URL:'http://127.0.0.1:18088',JWT_SECRET:'devstore_browser_test_jwt_32_bytes_secret',CSRF_SECRET:'devstore_browser_test_csrf_32_bytes_secret',NUXT_API_INTERNAL_URL:'http://127.0.0.1:18080',TRUSTED_PROXIES:'127.0.0.1',NUXT_TELEMETRY_DISABLED:'1'};
execFileSync('cargo',['build','--workspace'],{env,stdio:'inherit'});
const credentials={email:'admin-'+randomUUID()+'@example.test',password:'Browser-fixture-only-42!'};
writeFileSync('.runtime/e2e.json',JSON.stringify(credentials),{mode:0o600});
writeFileSync('.runtime/e2e-payment.json',JSON.stringify({channel:'bank',payee:'浏览器测试 · 请勿转账',account:'TEST-ONLY-NOT-A-REAL-ACCOUNT',instructions:'仅用于自动化验收，禁止转账真实资金。'}));
execFileSync('target/debug/admin',['init-storage'],{env,stdio:'inherit'});
execFileSync('target/debug/admin',['create','--email',credentials.email,'--password-stdin'],{env,input:credentials.password,stdio:['pipe','pipe','inherit']});
execFileSync('target/debug/admin',['seed-demo','--admin-email',credentials.email],{env,stdio:'inherit'});
execFileSync('target/debug/admin',['payment-config','--admin-email',credentials.email,'--file','.runtime/e2e-payment.json'],{env,stdio:'inherit'});
const children=[];
function start(command,args,log){
 const fd=openSync('.runtime/'+log,'w');const child=spawn(command,args,{cwd:root,env,stdio:['ignore',fd,fd]});children.push(child);return child;
}
start('target/debug/devstore-server',[],'e2e-api.log');
if(process.env.E2E_PRODUCTION==='1')start(process.execPath,['apps/web/.output/server/index.mjs'],'e2e-web.log');
else start(process.execPath,['node_modules/nuxt/bin/nuxt.mjs','dev','apps/web','--host','0.0.0.0','--port','13000'],'e2e-web.log');
async function wait(url){
 for(let attempt=0;attempt<120;attempt++){try{if((await fetch(url)).ok)return;}catch{}await new Promise(r=>setTimeout(r,1000));}
 throw new Error('Service failed to start: '+url);
}
let stopping=false;
function stop(){
 if(stopping)return;stopping=true;
 for(const child of children)child.kill('SIGTERM');
 try{execFileSync('docker',[...compose,'--profile','e2e','stop','gateway'],{stdio:'pipe'});}catch{}
 // The random schema belongs only to this test run.
 try{execFileSync('docker',[...compose,'exec','-T','postgres','psql','-U','devstore_test','-d','devstore_test','-c','DROP SCHEMA '+schema+' CASCADE'],{stdio:'pipe'});}catch{}
 process.exit(0);
}
process.on('SIGTERM',stop);process.on('SIGINT',stop);
try{
 await wait('http://127.0.0.1:18080/api/v1/health/ready');
 await wait('http://127.0.0.1:13000');
 execFileSync('docker',[...compose,'--profile','e2e','up','-d','gateway'],{stdio:'inherit'});
 console.log('E2E gateway ready on http://127.0.0.1:18088');
}catch(error){console.error(error);stop();}
setInterval(()=>{},30000);
