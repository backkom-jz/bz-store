<script setup lang="ts">
import { computed } from 'vue';
import { RouterLink, useRoute } from 'vue-router';
import { logout, session } from './session';
const route = useRoute();
const title = computed(() => route.meta.title || '管理后台');
const groups = [
  { label: '日常交易', items: [{ path: '/payments', label: '付款审核', mark: '付' }, { path: '/orders', label: '订单管理', mark: '单' }, { path: '/tickets', label: '服务工单', mark: '服' }] },
  { label: '商品与内容', items: [{ path: '/products', label: '商品管理', mark: '商' }, { path: '/tutorials', label: '教程内容', mark: '教' }, { path: '/blog', label: '博客文章', mark: '文' }] },
  { label: '运营记录', items: [{ path: '/audit', label: '操作记录', mark: '记' }] },
];
function active(path: string) { return route.path === path || route.path.startsWith(path + '/'); }
</script>
<template>
  <a class="sr-only" href="#main">跳转到主要内容</a>
  <div v-if="route.name === 'login'" class="login-shell"><RouterView /></div>
  <div v-else class="admin-shell">
    <aside class="side">
      <RouterLink class="brand" to="/workbench"><span class="brand-mark" aria-hidden="true">D</span><span>DevStore<small>运营管理后台</small></span></RouterLink>
      <nav aria-label="后台导航">
        <RouterLink to="/workbench" :class="{ 'is-active': active('/workbench') }"><span class="nav-mark" aria-hidden="true">台</span>运营工作台</RouterLink>
        <section v-for="group in groups" :key="group.label" class="nav-group" :aria-label="group.label"><p class="nav-label">{{ group.label }}</p><RouterLink v-for="item in group.items" :key="item.path" :to="item.path" :class="{ 'is-active': active(item.path) }"><span class="nav-mark" aria-hidden="true">{{ item.mark }}</span>{{ item.label }}</RouterLink></section>
      </nav>
      <div class="side-foot"><span class="badge">管理员</span><p class="muted wrap">{{ session.user?.email }}</p><div class="side-foot-links"><a href="/" target="_blank" rel="noopener">查看商城 ↗</a><button class="text-button" type="button" @click="logout">退出登录</button></div></div>
    </aside>
    <div class="admin-workspace"><header class="workspace-header"><div class="breadcrumbs"><RouterLink to="/workbench">管理后台</RouterLink><span>/</span><span>{{ title }}</span></div><span class="manual-mode">● 人工收款与交付</span></header><main id="main"><RouterView /></main><footer class="workspace-footer">DevStore · 用清晰的记录，完成每一次交付。</footer></div>
  </div>
</template>
