export default defineNuxtRouteMiddleware(to=>{
 if(import.meta.server)return;
 const {user,error}=useSession();
 if(!user.value&&!error.value)return navigateTo({path:'/login',query:{returnTo:to.fullPath}});
});
