<script setup lang="ts">
import { useRoute } from 'vue-router';
import { logout, session } from './session';

const route = useRoute();
function navActive(prefix: string) {
  return route.path === prefix || route.path.startsWith(prefix + '/');
}
</script>

<template>
  <a class="sr-only" href="#main">跳转到主要内容</a>
  <div v-if="route.name === 'login'" class="login-shell">
    <RouterView />
  </div>
  <div v-else class="admin-shell">
    <aside class="side">
      <a class="brand" href="/">DevStore</a>
      <p class="eyebrow">管理后台</p>
      <nav aria-label="后台导航">
        <RouterLink to="/payments" :class="{ 'is-active': navActive('/payments') }">付款审核</RouterLink>
        <RouterLink to="/orders" :class="{ 'is-active': navActive('/orders') }">订单</RouterLink>
        <RouterLink to="/products" :class="{ 'is-active': navActive('/products') }">商品</RouterLink>
        <RouterLink to="/tutorials" :class="{ 'is-active': navActive('/tutorials') }">教程</RouterLink>
        <RouterLink to="/audit" :class="{ 'is-active': navActive('/audit') }">审计</RouterLink>
      </nav>
      <div class="side-foot">
        <p class="muted wrap">{{ session.user?.email }}</p>
        <button class="btn secondary small" type="button" @click="logout">退出</button>
      </div>
    </aside>
    <main id="main">
      <RouterView />
    </main>
  </div>
</template>
