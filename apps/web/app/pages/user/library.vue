<script setup lang="ts">
import type { LibraryEntry, Page } from '@devstore/shared-types';
import { statusLabel } from '@devstore/ui/format';
import { ArrowRight, BookOpen, Download, MessagesSquare } from '@lucide/vue';

definePageMeta({ middleware: 'auth' });
useSeoMeta({ title: '我的资源 · DevStore' });
const api = useApi();
const { user, error: sessionError } = useSession();
const page = ref(1);
const message = ref('');
const { data, error, status, refresh } = await useAsyncData(
  () => 'private:library:' + (user.value?.id || 'none') + ':' + page.value,
  () => api.request<Page<LibraryEntry>>('/me/library?page=' + page.value),
  { server: false },
);

async function download(item: LibraryEntry) {
  if (!item.file_id) return;
  message.value = '';
  try {
    const blob = await api.request<Blob>('/me/downloads/' + item.file_id, { responseType: 'blob' });
    const url = URL.createObjectURL(blob);
    const link = document.createElement('a');
    link.href = url;
    link.download = item.slug;
    link.click();
    URL.revokeObjectURL(url);
  } catch (reason) {
    message.value = reason instanceof Error ? reason.message : '下载失败，请重试';
  }
}
</script>

<template>
  <div class="container">
    <div class="page-head">
      <span class="eyebrow">YOUR KNOWLEDGE LIBRARY</span>
      <h1>我的资源</h1>
      <p class="muted">已开通的教程、数字文件和服务都在这里。</p>
    </div>
    <UserNav />
    <ClientOnly>
      <p v-if="sessionError" class="error">{{ sessionError }}</p>
      <p v-if="message" class="error" role="alert">{{ message }}</p>
      <ApiFeedback :pending="status === 'pending'" :error="error" @retry="refresh" />
      <template v-if="data && !error">
        <div v-if="data.items.length" class="product-grid">
          <article v-for="item in data.items" :key="item.product_id" class="panel">
            <BookOpen :size="28" style="color:var(--cyan);margin-bottom:20px" />
            <span class="badge">{{ statusLabel[item.type] || item.type }}</span>
            <h2 style="font-size:19px">{{ item.title }}</h2>
            <NuxtLink v-if="item.type === 'tutorial' && item.slug" class="text-link row" style="font-size:13px" :to="'/tutorials/' + item.slug">继续阅读 <ArrowRight :size="15" /></NuxtLink>
            <button v-else-if="item.type === 'digital' && item.file_id" class="text-link row" style="font-size:13px;background:none" type="button" @click="download(item)">下载文件 <Download :size="15" /></button>
            <NuxtLink v-else-if="item.type === 'service'" class="text-link row" style="font-size:13px" to="/user/tickets">查看工单 <MessagesSquare :size="15" /></NuxtLink>
          </article>
        </div>
        <div v-else class="empty">
          <BookOpen :size="32" style="margin:0 auto 18px" />
          <h2 style="font-size:21px;color:var(--ink)">还没有已购内容</h2>
          <NuxtLink class="btn" to="/products">去看看商品 <ArrowRight :size="16" /></NuxtLink>
        </div>
      </template>
    </ClientOnly>
  </div>
</template>
