import {test as base,expect,request,type APIRequestContext,type Page} from '@playwright/test';
import {execFileSync} from 'node:child_process';
import {readFileSync} from 'node:fs';
export const origin='http://127.0.0.1:18088';
export const receiptPng=Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==','base64');
export function credentials(){return JSON.parse(readFileSync('.runtime/e2e.json','utf8')) as {email:string;password:string};}
export const test=base.extend<{resetLimits:void}>({resetLimits:[async({},use)=>{
 execFileSync('docker',['compose','-f','deploy/docker-compose.test.yml','exec','-T','redis','redis-cli','FLUSHDB'],{stdio:'pipe'});
 await use();
},{auto:true}]});
export {expect};
export function hydrated(page:Page){
 return page.waitForFunction(()=>Boolean((document.querySelector('#__nuxt') as (HTMLElement & {__vue_app__?:unknown}) | null)?.__vue_app__));
}
export async function adminApi(){
 const api=await request.newContext({baseURL:origin,extraHTTPHeaders:{Origin:origin}});
 const login=await api.post('/api/v1/auth/login',{data:credentials()});expect(login.status()).toBe(200);
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
