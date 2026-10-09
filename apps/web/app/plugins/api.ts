import {createApiClient,ApiError} from '@devstore/api-client';
import type {PublicUser} from '@devstore/shared-types';
export default defineNuxtPlugin(async(nuxt)=>{
 const config=useRuntimeConfig();const user=useState<PublicUser|null>('session',()=>null);
 const sessionError=useState<string>('session-error',()=>'');
 const clearPrivate=()=>nuxt.runWithContext(()=>clearNuxtData(key=>key.startsWith('private:')));
 const api=createApiClient({baseUrl:import.meta.server?config.apiInternalUrl+'/api/v1':'/api/v1',browser:import.meta.client,cookie:import.meta.server?useRequestHeaders(['cookie']).cookie:undefined,onUnauthorized(){if(!user.value)return;user.value=null;clearPrivate();}});
 const load=async()=>{
  try{const next=await api.request<PublicUser>('/users/me');if(user.value?.id!==next.id)clearPrivate();user.value=next;sessionError.value='';}
  catch(error){if(error instanceof ApiError&&error.status===401){user.value=null;clearPrivate();}else sessionError.value=error instanceof Error?error.message:'暂时无法检查登录状态';}
 };
 await load();
 let announceSession=()=>{};
 if(import.meta.client){
  const channel=typeof BroadcastChannel!=='undefined'?new BroadcastChannel('devstore-session'):undefined;
  channel?.addEventListener('message',()=>{user.value=null;clearPrivate();location.reload();});
  window.addEventListener('pageshow',event=>{if(event.persisted){user.value=null;clearPrivate();location.reload();}});
  // Same user keeps the cashier and refetches it. A different user still drops private data.
  document.addEventListener('visibilitychange',()=>{
   if(document.visibilityState!=='visible')return;
   void (async()=>{
    const before=user.value?.id??null;
    await load();
    if(!user.value||user.value.id!==before)return;
    const keys=Object.keys(nuxt.payload.data).filter(key=>key.startsWith('private:'));
    if(keys.length)await nuxt.runWithContext(()=>refreshNuxtData(keys));
   })();
  });
  announceSession=()=>channel?.postMessage('changed');
 }
 return {provide:{api,reloadSession:load,announceSession}};
});
