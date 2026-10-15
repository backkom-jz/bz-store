<script setup lang="ts">
import {Menu,X,ArrowUpRight,LogOut} from '@lucide/vue';
const {user,logout}=useSession();const open=ref(false);const message=ref('');
const route=useRoute();watch(()=>route.fullPath,()=>open.value=false);
async function exit(){try{await logout();}catch(error){message.value=error instanceof Error?error.message:'退出失败，请重试';}}
</script>
<template>
 <a class="sr-only" href="#main">跳转到主要内容</a>
 <header class="site-header"><div class="container header-inner">
  <NuxtLink class="brand" to="/" aria-label="DevStore 首页"><span class="brand-symbol">&lt;/&gt;</span><span>Dev<span>Store</span></span></NuxtLink>
  <nav aria-label="主要导航" class="nav-links" :class="{open}"><NuxtLink to="/tutorials">教程中心</NuxtLink><NuxtLink to="/products">商品商城</NuxtLink><NuxtLink to="/blog">博客</NuxtLink><NuxtLink to="/about">关于我</NuxtLink></nav>
  <div class="header-actions"><template v-if="user"><a v-if="user.is_admin" href="/admin/">管理后台</a><NuxtLink to="/user/library">我的学习</NuxtLink><button class="text-link" style="background:none" aria-label="退出登录" @click="exit"><LogOut :size="17"/></button></template><template v-else><NuxtLink to="/login">登录</NuxtLink><NuxtLink to="/register" class="btn small">开始学习 <ArrowUpRight :size="14"/></NuxtLink></template>
  <button class="mobile-toggle" :aria-expanded="open" aria-label="切换导航" @click="open=!open"><X v-if="open" :size="21"/><Menu v-else :size="21"/></button></div>
 </div></header>
 <div v-if="message" class="container"><p role="alert" class="error">{{ message }}</p></div>
 <main id="main" class="site-main"><slot/></main>
 <footer class="site-footer"><div class="container spread"><div><NuxtLink class="brand" to="/"><span class="brand-symbol">&lt;/&gt;</span><span>Dev<span>Store</span></span></NuxtLink><p style="margin:12px 0 0">用代码解决问题，让知识创造价值。</p></div><div><div class="footer-links"><NuxtLink to="/tutorials">探索教程</NuxtLink><NuxtLink to="/blog">博客</NuxtLink><NuxtLink to="/user/orders">我的订单</NuxtLink><NuxtLink to="/about">关于 DevStore</NuxtLink></div><p style="margin:15px 0 0">© {{ new Date().getFullYear() }} DevStore · Built with Rust & Vue</p></div></div></footer>
</template>
