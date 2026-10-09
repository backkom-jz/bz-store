<script setup lang="ts">
import type { Page, TutorialSummary } from '@devstore/shared-types';
import { money, statusLabel } from '@devstore/ui/format';
import { onMounted, ref } from 'vue';
import { RouterLink } from 'vue-router';
import { api } from '../session';

const q = ref('');
const page = ref(1);
const data = ref<Page<TutorialSummary> | null>(null);
const error = ref('');
const pending = ref(false);

async function load() {
  pending.value = true;
  error.value = '';
  const params = new URLSearchParams({ page: String(page.value), page_size: '20' });
  if (q.value.trim()) params.set('q', q.value.trim());
  try {
    data.value = await api.request<Page<TutorialSummary>>('/admin/tutorials?' + params);
  } catch (reason) {
    error.value = reason instanceof Error ? reason.message : '加载失败，请重试';
  } finally {
    pending.value = false;
  }
}

function search() {
  page.value = 1;
  void load();
}

onMounted(() => void load());
</script>

<template>
  <header class="page-head">
    <h1>教程</h1>
    <p class="muted">从商品页创建教程。章节默认不公开，只有勾选预览后未购买用户才能阅读。</p>
  </header>
  <form class="toolbar" @submit.prevent="search">
    <label class="sr-only" for="tutorial-q">搜索教程标题</label>
    <input id="tutorial-q" v-model="q" placeholder="教程标题" maxlength="100">
    <button class="btn secondary" type="submit">搜索</button>
  </form>
  <p v-if="pending" class="muted" role="status">正在加载…</p>
  <div v-else-if="error" class="error" role="alert">
    <p>{{ error }}</p>
    <button class="btn secondary small" type="button" @click="load">重试</button>
  </div>
  <template v-else-if="data">
    <div v-if="data.items.length" class="table-wrap">
      <table>
        <thead>
          <tr><th>教程</th><th>价格</th><th>状态</th><th></th></tr>
        </thead>
        <tbody>
          <tr v-for="tutorial in data.items" :key="tutorial.id">
            <td>
              <strong>{{ tutorial.title }}</strong>
              <p class="mono wrap">{{ tutorial.slug }}</p>
            </td>
            <td>{{ money(tutorial.price_cents) }}</td>
            <td><span class="badge" :class="{ green: tutorial.status === 'published' }">{{ statusLabel[tutorial.status] }}</span></td>
            <td><RouterLink class="text-link" :to="'/tutorials/' + tutorial.id">编辑</RouterLink></td>
          </tr>
        </tbody>
      </table>
    </div>
    <div v-else class="empty"><h2>还没有教程</h2><p>打开一个商品，使用「创建教程」开始写章节。</p></div>
    <div v-if="data.total > data.page_size" class="pagination">
      <button class="btn secondary small" type="button" :disabled="page === 1" @click="page--; load()">上一页</button>
      <span>{{ page }}</span>
      <button class="btn secondary small" type="button" :disabled="page * data.page_size >= data.total" @click="page++; load()">下一页</button>
    </div>
  </template>
</template>
