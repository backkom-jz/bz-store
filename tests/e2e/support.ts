import {test as base,expect,request,type APIRequestContext,type Page} from '@playwright/test';
import {execFileSync} from 'node:child_process';
import {readFileSync} from 'node:fs';
import {redisCliInvocation} from '../../scripts/e2e-config.mjs';
import {claimAdminCounter} from '../../scripts/e2e-admin-state.mjs';
const configuredOrigin=process.env.E2E_BASE_URL;
const mailbox=process.env.TEST_MAILBOX_API_URL;
const redisValue=process.env.REDIS_TEST_URL;
const dbValue=process.env.MYSQL_TEST_DATABASE_URL;
if(process.env.DEVSTORE_TEST_ENV!=='cloud'||configuredOrigin!=='http://127.0.0.1:28088'||!mailbox||!redisValue||!dbValue||new URL(dbValue).pathname!=='/bz_ztore_test')throw new Error('E2E requires validated isolated cloud test settings');
export const origin:string=configuredOrigin;
const redisCommand=redisCliInvocation(redisValue,process.env);
const mailboxBase=mailbox.replace(/\/$/,'');
const mailboxApi=mailboxBase.endsWith('/api')?mailboxBase+'/v1':mailboxBase+'/api/v1';
export const receiptPng=Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==','base64');
let lastAdminCounter=0;
export function credentials(){const value=JSON.parse(readFileSync('.runtime/e2e.json','utf8')) as {email:string;password:string;totpSecret:string;mfaCounter:number};lastAdminCounter=Math.max(lastAdminCounter,value.mfaCounter);return value;}
export async function nextTotpCode(secret:string){
 const alphabet='ABCDEFGHIJKLMNOPQRSTUVWXYZ234567';let bits=0,acc=0;const bytes:number[]=[];for(const char of secret){acc=(acc<<5)|alphabet.indexOf(char);bits+=5;if(bits>=8){bits-=8;bytes.push((acc>>bits)&255);}}
 let counter=Math.floor(Date.now()/30000);while(counter<=lastAdminCounter || !claimAdminCounter('.runtime/e2e.json',counter)){await new Promise(resolve=>setTimeout(resolve,Math.max(100,30000-(Date.now()%30000)+100)));counter=Math.floor(Date.now()/30000);}
 const data=Buffer.alloc(8);data.writeBigUInt64BE(BigInt(counter));const hmac=await import('node:crypto').then(({createHmac})=>createHmac('sha1',Buffer.from(bytes)).update(data).digest());const offset=hmac[19]&15;const value=(((hmac[offset]&127)<<24)|(hmac[offset+1]<<16)|(hmac[offset+2]<<8)|hmac[offset+3])%1000000;lastAdminCounter=counter;return String(value).padStart(6,'0');
}
export const test=base.extend<{resetLimits:void}>({resetLimits:[async({},use)=>{
 execFileSync('redis-cli',redisCommand.args,{stdio:'pipe',env:redisCommand.env});
 await use();
},{auto:true}]});
export {expect};
export function hydrated(page:Page){
 return page.waitForFunction(()=>Boolean((document.querySelector('#__nuxt') as (HTMLElement & {__vue_app__?:unknown}) | null)?.__vue_app__));
}
export async function verifyMailbox(email:string,post:(path:string,data:unknown)=>Promise<{status:()=>number;text:()=>Promise<string>}>){
 const deadline=Date.now()+30000;let token='';
 while(Date.now()<deadline&&!token){
  const index=await fetch(mailboxApi+'/messages');
  if(index.ok){const body=await index.json() as {messages?:Array<{ID:string;To?:Array<{Address:string}>}>};
   const message=body.messages?.find(item=>item.To?.some(to=>to.Address.toLowerCase()===email.toLowerCase()));
   if(message){const detail=await fetch(mailboxApi+'/message/'+encodeURIComponent(message.ID));if(detail.ok){const mail=await detail.json() as {Text?:string};token=mail.Text?.match(/verify-email\?token=([^\s]+)/)?.[1]??'';}}
  }
  if(!token)await new Promise(resolve=>setTimeout(resolve,250));
 }
 if(!token)throw new Error('verification email did not arrive in the configured test mailbox');
 const response=await post('/api/v1/auth/verify-email',{token});expect(response.status(),await response.text()).toBe(200);
}
export async function adminApi(){
 const api=await request.newContext({baseURL:origin,extraHTTPHeaders:{Origin:origin}});
 const admin=credentials();const login=await api.post('/api/v1/auth/login',{data:{email:admin.email,password:admin.password,totp_code:await nextTotpCode(admin.totpSecret)}});expect(login.status(),await login.text()).toBe(200);
 const csrf=await api.get('/api/v1/auth/csrf');const token=(await csrf.json()).data.csrf_token as string;
 return {api,write:async(method:'post'|'patch',path:string,data:unknown)=>{const response=await api[method]('/api/v1'+path,{data,headers:{'X-CSRF-Token':token}});expect(response.ok(),await response.text()).toBeTruthy();return (await response.json()).data;}};
}
export async function makeCourse(admin:Awaited<ReturnType<typeof adminApi>>,content='Paid browser-only secret'){
 const slug='e2e-'+crypto.randomUUID();
 const product=await admin.write('post','/admin/products',{title:'浏览器验收教程',slug,type:'tutorial',price_cents:9900,description:'围绕真实项目学习 Rust API 与数据库事务。'});
 const tutorial=await admin.write('post','/admin/tutorials',{title:product.title,slug,product_id:product.id});
 const preview=await admin.write('post','/admin/tutorials/'+tutorial.id+'/chapters',{title:'01 免费预览',content_md:'# 开始学习\n\n这是一段公开的预览。',is_preview:true,status:'published',sort_order:0});
 const paid=await admin.write('post','/admin/tutorials/'+tutorial.id+'/chapters',{title:'02 事务与授权',content_md:content,status:'published',sort_order:1});
 await admin.write('patch','/admin/tutorials/'+tutorial.id,{status:'published'});
 await admin.write('patch','/admin/products/'+product.id,{status:'published'});
 return {product,tutorial,preview,paid,slug};
}
