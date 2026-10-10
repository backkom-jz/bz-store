import {spawn,execFileSync} from 'node:child_process';
import {mkdirSync,writeFileSync,openSync} from 'node:fs';
import {randomUUID} from 'node:crypto';
const root=process.cwd();
if(!process.env.MYSQL_TEST_DATABASE_URL)throw new Error('MYSQL_TEST_DATABASE_URL must target the isolated bz_ztore_test database');
mkdirSync('.runtime',{recursive:true});mkdirSync('apps/admin/dist',{recursive:true});
const compose=['compose','-f','deploy/docker-compose.test.yml'];
execFileSync('docker',[...compose,'--profile','e2e','up','-d','--wait','--wait-timeout','120','redis','storage','mailpit'],{stdio:'inherit'});
const env={...process.env,APP_ENV:'test',APP_PORT:'18080',DATABASE_URL:process.env.MYSQL_TEST_DATABASE_URL,REDIS_URL:'redis://127.0.0.1:16379',S3_ENDPOINT:'http://127.0.0.1:19000',S3_ACCESS_KEY:'devstore_test',S3_SECRET_KEY:'devstore_test_only',S3_BUCKET:'devstore-test',SITE_URL:'http://127.0.0.1:18088',JWT_SECRET:'devstore_browser_test_jwt_32_bytes_secret',CSRF_SECRET:'devstore_browser_test_csrf_32_bytes_secret',SMTP_HOST:'127.0.0.1',SMTP_PORT:'11025',SMTP_TLS:'false',EMAIL_FROM:'DevStore Test <test@example.test>',EMAIL_OUTBOX_ENCRYPTION_KEY:Buffer.alloc(32,17).toString('base64'),NUXT_API_INTERNAL_URL:'http://127.0.0.1:18080',TRUSTED_PROXIES:'127.0.0.1',NUXT_TELEMETRY_DISABLED:'1'};
execFileSync('cargo',['build','--workspace'],{env,stdio:'inherit'});
const credentials={email:'admin-'+randomUUID()+'@example.test',password:'Browser-fixture-only-42!'};
execFileSync('target/debug/admin',['create','--email',credentials.email,'--password-stdin'],{env,input:credentials.password,stdio:['pipe','pipe','inherit']});
const mfaOutput=execFileSync('target/debug/admin',['mfa-begin','--admin-email',credentials.email],{env,encoding:'utf8'});
const totpSecret=mfaOutput.match(/Authenticator secret \(shown once\): ([A-Z2-7]+)/)?.[1];
if(!totpSecret)throw new Error('could not read one-time E2E MFA seed');
function totp(secret,counter){
 const alphabet='ABCDEFGHIJKLMNOPQRSTUVWXYZ234567';let bits=0,acc=0;const bytes=[];for(const char of secret){acc=(acc<<5)|alphabet.indexOf(char);bits+=5;if(bits>=8){bits-=8;bytes.push((acc>>bits)&255);}}const key=Buffer.from(bytes);const data=Buffer.alloc(8);data.writeBigUInt64BE(BigInt(counter));const digest=createHmac('sha1',key).update(data).digest();const offset=digest[19]&15;return String((((digest[offset]&127)<<24)|(digest[offset+1]<<16)|(digest[offset+2]<<8)|digest[offset+3])%1000000).padStart(6,'0');
}
const {createHmac}=await import('node:crypto');
const current=Math.floor(Date.now()/30000);const setupCode=totp(totpSecret,current);
execFileSync('target/debug/admin',['mfa-confirm','--admin-email',credentials.email,'--code-stdin'],{env,input:setupCode,stdio:['pipe','inherit','inherit']});
writeFileSync('.runtime/e2e.json',JSON.stringify({...credentials,totpSecret,mfaCounter:current}),{mode:0o600});
writeFileSync('.runtime/e2e-payment.json',JSON.stringify({channel:'bank',payee:'浏览器测试 · 请勿转账',account:'TEST-ONLY-NOT-A-REAL-ACCOUNT',instructions:'仅用于自动化验收，禁止转账真实资金。'}));
execFileSync('target/debug/admin',['init-storage'],{env,stdio:'inherit'});
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
