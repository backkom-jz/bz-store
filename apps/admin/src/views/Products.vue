<script setup lang="ts">
import type { Page, ProductSummary } from '@devstore/shared-types';
import { money, statusLabel } from '@devstore/ui/format';
import { onMounted, ref } from 'vue';
import { RouterLink } from 'vue-router';
import { api } from '../session';

const q = ref('');
const page = ref(1);
const data = ref<Page<ProductSummary> | null>(null);
const error = ref('');
const pending = ref(false);

async function load() {
  pending.value = true;
  error.value = '';
  const params = new URLSearchParams({ page: String(page.value), page_size: '20' });
  if (q.value.trim()) params.set('q', q.value.trim());
  try {
    data.value = await api.request<Page<ProductSummary>>('/admin/products?' + params);
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
  <header class="page-head spread">
    <div>
      <h1>商品</h1>
      <p class="muted">先保存草稿，再编写教程章节。发布前必须有已发布的付费章节。</p>
    </div>
    <RouterLink class="btn" to="/products/new">新建商品</RouterLink>
  </header>
  <form class="toolbar" @submit.prevent="search">
    <label class="sr-only" for="product-q">搜索商品标题</label>
    <input id="product-q" v-model="q" placeholder="商品标题" maxlength="100">
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
          <tr><th>商品</th><th>价格</th><th>状态</th><th></th></tr>
        </thead>
        <tbody>
          <tr v-for="product in data.items" :key="product.id">
            <td>
              <strong>{{ product.title }}</strong>
              <p class="mono wrap">{{ product.slug }}</p>
            </td>
            <td>{{ money(product.price_cents) }}</td>
            <td>
              <span class="badge" :class="{ green: product.status === 'published' }">{{ statusLabel[product.status] }}</span>
              <span v-if="product.is_demo" class="badge amber">示例</span>
            </td>
            <td><RouterLink class="text-link" :to="'/products/' + product.id">编辑</RouterLink></td>
          </tr>
        </tbody>
      </table>
    </div>
    <div v-else class="empty"><h2>没有商品</h2><p>新建一个教程商品后，才能编写章节并上架。</p></div>
    <div v-if="data.total > data.page_size" class="pagination">
      <button class="btn secondary small" type="button" :disabled="page === 1" @click="page--; load()">上一页</button>
      <span>{{ page }}</span>
      <button class="btn secondary small" type="button" :disabled="page * data.page_size >= data.total" @click="page++; load()">下一页</button>
    </div>
  </template>
</template>
