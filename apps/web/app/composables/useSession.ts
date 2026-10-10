import type {PublicUser} from '@devstore/shared-types';
export function useSession(){
 const user=useState<PublicUser|null>('session',()=>null);
 const error=useState<string>('session-error',()=>'');
 const nuxt=useNuxtApp();const api=useApi();
 async function login(email:string,password:string,register=false){
  if(register){const accepted=await api.request<{accepted:boolean}>('/auth/register',{method:'POST',body:{email,password}});return accepted;}
  const next=await api.request<PublicUser>('/auth/login',{method:'POST',body:{email,password}});
  clearNuxtData(key=>key.startsWith('private:'));user.value=next;error.value='';nuxt.$announceSession?.();return next;
 }
 async function logout(){
  await api.request('/auth/logout',{method:'POST'});
  user.value=null;clearNuxtData(key=>key.startsWith('private:'));nuxt.$announceSession?.();
  if(import.meta.client)location.replace('/');
 }
 return {user,error,login,logout,reload:nuxt.$reloadSession};
}
