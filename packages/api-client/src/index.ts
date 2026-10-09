import type {ApiEnvelope} from '@devstore/shared-types';
export class ApiError extends Error{
 constructor(public status:number,public code:string,message:string,public requestId=''){super(message);this.name='ApiError';}
}
export interface RequestOptions{method?:string;body?:unknown;headers?:HeadersInit;signal?:AbortSignal;responseType?:'json'|'blob'}
export interface ApiClientOptions{baseUrl:string;fetch?:typeof fetch;browser?:boolean;cookie?:string;onUnauthorized?:()=>void}
export interface ApiClient{request<T>(path:string,options?:RequestOptions):Promise<T>;clear():void}
export function createApiClient(options:ApiClientOptions):ApiClient{
 const transport=options.fetch??globalThis.fetch.bind(globalThis);
 const browser=options.browser??typeof window!=='undefined';
 let csrf='';let csrfFlight:Promise<string>|undefined;let refreshFlight:Promise<void>|undefined;
 const clear=()=>{csrf='';csrfFlight=undefined;};
 async function decode<T>(response:Response,blob=false):Promise<T>{
  if(blob&&response.ok)return await response.blob() as T;
  let payload:ApiEnvelope<T>;
  try{payload=await response.json();}catch{throw new ApiError(response.status,'BAD_RESPONSE','服务暂时不可用，请稍后重试');}
  if(!response.ok)throw new ApiError(response.status,payload.code,payload.message,payload.request_id);
  return payload.data;
 }
 async function token():Promise<string>{
  if(csrf)return csrf;
  csrfFlight??=send('/auth/csrf',{},false).then(r=>decode<{csrf_token:string}>(r)).then(data=>{csrf=data.csrf_token;return csrf;}).finally(()=>{csrfFlight=undefined;});
  return csrfFlight;
 }
 async function send(path:string,input:RequestOptions,protect=true):Promise<Response>{
  if(!path.startsWith('/')||path.startsWith('//')||path.includes('\\')||/[\u0000-\u001f]/.test(path))throw new Error('Invalid API path');
  const method=(input.method??'GET').toUpperCase();
  const headers=new Headers(input.headers);
  if(!browser&&options.cookie)headers.set('Cookie',options.cookie);
  const mutation=!['GET','HEAD','OPTIONS'].includes(method);
  if(mutation&&protect&&!['/auth/login','/auth/register'].includes(path))headers.set('X-CSRF-Token',await token());
  const isForm=typeof FormData!=='undefined'&&input.body instanceof FormData;
  if(input.body!==undefined&&!isForm)headers.set('Content-Type','application/json');
  return transport(options.baseUrl+path,{method,credentials:'include',cache:'no-store',headers,body:input.body===undefined?undefined:isForm?input.body as FormData:JSON.stringify(input.body),signal:input.signal});
 }
 async function recover():Promise<void>{
  if(!refreshFlight){
   const rotate=async()=>{
    // Another tab may have already replaced the shared HttpOnly cookie.
    const current=await send('/users/me',{},false);
    if(current.ok){clear();return;}
    if(current.status!==401)throw new ApiError(current.status,'UNAVAILABLE','暂时无法检查登录状态');
    try{
     const csrfToken=await token();
     await decode(await send('/auth/refresh',{method:'POST',headers:{'X-CSRF-Token':csrfToken}},false));
     clear();
    }catch(error){
     if(error instanceof ApiError&&error.status===401){
      // Covers browsers without Web Locks and concurrent cookie rotation.
      for(const delay of [50,150]){
       await new Promise(resolve=>setTimeout(resolve,delay));
       if((await send('/users/me',{},false)).ok){clear();return;}
      }
      clear();options.onUnauthorized?.();
     }
     throw error;
    }
   };
   const locks=browser&&typeof navigator!=='undefined'?navigator.locks:undefined;
   refreshFlight=(async()=>{if(locks)await locks.request('devstore-session-refresh',rotate);else await rotate();})().finally(()=>{refreshFlight=undefined;});
  }
  return refreshFlight;
 }
 async function request<T>(path:string,input:RequestOptions={}):Promise<T>{
  const run=async()=>decode<T>(await send(path,input),input.responseType==='blob');
  try{
   const result=await run();
   if(['/auth/login','/auth/register','/auth/logout'].includes(path))clear();
   return result;
  }catch(error){
   if(error instanceof ApiError&&error.status===401&&browser&&!path.startsWith('/auth/')){
    await recover();
    try{return await run();}catch(retryError){
     if(retryError instanceof ApiError&&retryError.status===401){clear();options.onUnauthorized?.();}
     throw retryError;
    }
   }
   throw error;
  }
 }
 return {request,clear};
}
export function safeReturnTo(value:unknown):string{
 if(typeof value!=='string'||!value.startsWith('/')||value.startsWith('//')||value.includes('\\')||/[%\u0000-\u0020]/.test(value))return '/';
 return value;
}
