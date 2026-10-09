import type {OrderDetail} from '@devstore/shared-types';
export function usePurchase(){
 const {user}=useSession();const api=useApi();const route=useRoute();const pending=ref(false);const error=ref('');
 async function buy(product:string){
  if(!user.value){await navigateTo({path:'/login',query:{returnTo:route.fullPath}});return;}
  if(pending.value)return;pending.value=true;error.value='';
  const storageKey='devstore-order:'+user.value.id+':'+product;
  let key=sessionStorage.getItem(storageKey);if(!key){key=crypto.randomUUID();sessionStorage.setItem(storageKey,key);}
  try{const order=await api.request<OrderDetail>('/orders',{method:'POST',body:{product_id:product,quantity:1},headers:{'Idempotency-Key':key}});sessionStorage.removeItem(storageKey);await navigateTo('/checkout/'+order.id);}
  catch(e){error.value=e instanceof Error?e.message:'下单失败，请重试';}finally{pending.value=false;}
 }
 return {buy,pending,error};
}
