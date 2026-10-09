import {describe,it,expect,vi} from 'vitest';
import {createApiClient,safeReturnTo} from './index';
const response=(status:number,data:unknown)=>new Response(JSON.stringify({code:status===200?'OK':'UNAUTHENTICATED',message:'message',data,request_id:'test'}),{status,headers:{'content-type':'application/json'}});
describe('API session recovery',()=>{
 it('shares one refresh and keeps checkout idempotency key',async()=>{
  let valid=false;let refreshes=0;const keys:string[]=[];
  const transport=vi.fn(async(url:RequestInfo|URL,init?:RequestInit)=>{
   const path=String(url);
   if(path.endsWith('/auth/csrf'))return response(200,{csrf_token:'csrf'});
   if(path.endsWith('/auth/refresh')){refreshes++;await new Promise(r=>setTimeout(r,10));valid=true;return response(200,{id:'user'});}
   if(path.endsWith('/users/me'))return response(valid?200:401,valid?{id:'user'}:null);
   if(path.endsWith('/orders')){keys.push(new Headers(init?.headers).get('Idempotency-Key')!);return response(valid?200:401,{id:'order'});}
   return response(valid?200:401,{id:'resource'});
  });
  const api=createApiClient({fetch:transport,baseUrl:'/api/v1',browser:true});
  await Promise.all([api.request('/orders',{method:'POST',body:{product_id:'p',quantity:1},headers:{'Idempotency-Key':'same-key-for-retry'}}),api.request('/me/library'),api.request('/users/me')]);
  expect(refreshes).toBe(1);expect(keys).toEqual(['same-key-for-retry','same-key-for-retry']);
 });
 it('clears session only after failed refresh and never recursively retries',async()=>{
  const expired=vi.fn();let refreshes=0;
  const api=createApiClient({baseUrl:'/api/v1',browser:true,onUnauthorized:expired,fetch:vi.fn(async(url:RequestInfo|URL)=>{
   if(String(url).endsWith('/auth/csrf'))return response(200,{csrf_token:'csrf'});
   if(String(url).endsWith('/auth/refresh'))refreshes++;
   return response(401,null);
  })});
  await expect(api.request('/orders')).rejects.toMatchObject({status:401});
  expect(expired).toHaveBeenCalledTimes(1);expect(refreshes).toBe(1);
 });
 it('does not refresh failed login and does not send server cookies elsewhere',async()=>{
  const transport=vi.fn(async()=>response(401,null));
  const api=createApiClient({baseUrl:'http://api:8080/api/v1',browser:false,cookie:'ds_access=private',fetch:transport});
  await expect(api.request('/auth/login',{method:'POST',body:{email:'a',password:'b'}})).rejects.toMatchObject({status:401});
  expect(transport).toHaveBeenCalledTimes(1);
  await expect(api.request('//attacker.example/steal')).rejects.toThrow();
  expect(transport).toHaveBeenCalledTimes(1);
 });
 it('rejects external and encoded return paths',()=>{
  for(const value of ['https://evil.test','//evil.test','/\\evil.test','/%2fevil.test','javascript:alert(1)',null])expect(safeReturnTo(value)).toBe('/');
  expect(safeReturnTo('/checkout/abc?source=login')).toBe('/checkout/abc?source=login');
 });
});
