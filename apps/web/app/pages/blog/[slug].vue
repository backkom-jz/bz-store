<script setup lang="ts">
import type { BlogPost } from '@devstore/shared-types';
import MarkdownContent from '@devstore/ui/MarkdownContent.vue';
import { dateTime } from '@devstore/ui/format';

const route = useRoute();
const api = useApi();
const { data, error, status, refresh } = await useAsyncData(
  () => 'blog-post:' + route.params.slug,
  () => api.request<BlogPost>('/blog/' + encodeURIComponent(String(route.params.slug))),
);
useSeoMeta({ title: () => (data.value ? data.value.title + ' · DevStore' : '博客 · DevStore'), description: () => data.value?.excerpt || 'DevStore 博客' });
</script>

<template>
  <div class="container">
    <div class="page-head">
      <div class="breadcrumbs"><NuxtLink to="/blog">博客</NuxtLink><span>/ 文章</span></div>
      <ApiFeedback :pending="status === 'pending'" :error="error" @retry="refresh" />
      <template v-if="data">
        <h1>{{ data.title }}</h1>
        <p v-if="data.published_at" class="muted">{{ dateTime(data.published_at) }}</p>
      </template>
    </div>
    <article v-if="data" class="panel">
      <MarkdownContent :content="data.content_md || ''" />
    </article>
  </div>
</template>
