<script setup lang="ts">
import type { BlogPost, Page } from '@devstore/shared-types';
import { dateTime, statusLabel } from '@devstore/ui/format';
import { onMounted, ref } from 'vue';
import { RouterLink } from 'vue-router';
import { api } from '../session';

const data = ref<Page<BlogPost> | null>(null);
const error = ref('');
const pending = ref(false);

async function load() {
  pending.value = true;
  error.value = '';
  try {
    data.value = await api.request<Page<BlogPost>>('/admin/blog');
  } catch (reason) {
    error.value = reason instanceof Error ? reason.message : '加载失败，请重试';
  } finally {
    pending.value = false;
  }
}

onMounted(() => void load());
</script>

<template>
  <div class="spread">
    <h1>博客</h1>
    <RouterLink class="btn" to="/blog/new">写文章</RouterLink>
  </div>
  <p v-if="pending" class="muted" role="status">正在加载…</p>
  <div v-else-if="error" class="error" role="alert">
    <p>{{ error }}</p>
    <button class="btn secondary small" type="button" @click="load">重试</button>
  </div>
  <table v-else-if="data" class="table">
    <thead><tr><th>标题</th><th>状态</th><th>更新时间</th><th></th></tr></thead>
    <tbody>
      <tr v-if="!data.items.length"><td colspan="4" class="muted">还没有文章。</td></tr>
      <tr v-for="post in data.items" :key="post.id">
        <td><strong>{{ post.title }}</strong><p class="mono wrap">{{ post.slug }}</p></td>
        <td><span class="badge">{{ statusLabel[post.status || 'draft'] }}</span></td>
        <td>{{ dateTime(post.updated_at || post.published_at || '') }}</td>
        <td><RouterLink class="text-link" :to="'/blog/' + post.id">编辑</RouterLink></td>
      </tr>
    </tbody>
  </table>
</template>
