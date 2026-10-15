<script setup lang="ts">
import type { BlogPost, Page } from '@devstore/shared-types';
import { dateTime } from '@devstore/ui/format';

useSeoMeta({ title: '博客 · DevStore', description: 'DevStore 的文章与更新' });
const api = useApi();
const page = ref(1);
const { data, error, status, refresh } = await useAsyncData(
  () => 'blog:' + page.value,
  () => api.request<Page<BlogPost>>('/blog?page=' + page.value),
);
</script>

<template>
  <div class="container">
    <div class="page-head">
      <span class="eyebrow">NOTES</span>
      <h1>博客</h1>
      <p class="muted">产品更新和写作，不进入商品目录。</p>
    </div>
    <ApiFeedback :pending="status === 'pending'" :error="error" @retry="refresh" />
    <div v-if="data?.items.length" class="stack">
      <NuxtLink v-for="post in data.items" :key="post.id" class="panel" :to="'/blog/' + post.slug">
        <h2 style="font-size:20px">{{ post.title }}</h2>
        <p class="muted">{{ post.excerpt }}</p>
        <p v-if="post.published_at" class="muted">{{ dateTime(post.published_at) }}</p>
      </NuxtLink>
    </div>
    <div v-else-if="data && !error" class="empty">还没有发布的文章。</div>
  </div>
</template>
