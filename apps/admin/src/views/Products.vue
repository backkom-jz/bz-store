<script setup lang="ts">
import type { Page, ProductSummary } from '@devstore/shared-types';
import { money, statusLabel } from '@devstore/ui/format';
import { RouterLink } from 'vue-router';
import { useAdminList } from '../composables/useAdminList';
const { q, page, data, error, pending, load, search, reset, goPage } = useAdminList<ProductSummary>('/admin/products');
const preparation: Record<string, string> = { tutorial: '准备教程和已发布章节', digital: '上传 PDF / ZIP 交付文件', service: '说明服务范围与交付方式' };
</script>

<template>
  <header class="page-head spread">
    <div>
      <p class="eyebrow">商品与内容</p><h1>商品</h1>
      <p class="muted">先保存草稿，再按商品类型准备交付内容。发布后用户才能购买。</p>
    </div>
    <RouterLink class="btn" to="/products/new">新建商品</RouterLink>
  </header>
  <section class="catalog-guide" aria-label="商品交付说明"><div><strong>教程</strong><p>保存草稿 → 编写并发布章节 → 发布商品</p></div><div><strong>数字商品</strong><p>保存草稿 → 上传 PDF / ZIP → 发布商品</p></div><div><strong>服务</strong><p>说明交付范围 → 发布商品 → 收款后进入工单</p></div></section>
  <form class="toolbar filter-panel" @submit.prevent="search">
    <label for="product-q">搜索商品标题</label>
    <input id="product-q" v-model="q" placeholder="商品标题" maxlength="100">
    <button class="btn" type="submit">搜索</button><button class="btn secondary" type="button" @click="reset">重置</button>
  </form>
  <p v-if="pending" class="muted" role="status">正在加载…</p>
  <div v-else-if="error" class="error" role="alert">
    <p>{{ error }}</p>
    <button class="btn secondary small" type="button" @click="load">重试</button>
  </div>
  <template v-else-if="data">
    <div class="list-summary"><span>共 <strong>{{ data.total }}</strong> 件商品</span><span class="muted">第 {{ page }} 页</span></div>
    <div v-if="data.items.length" class="table-wrap panel-table">
      <table>
        <thead>
          <tr><th>商品</th><th>类型与准备事项</th><th>价格</th><th>状态</th><th>操作</th></tr>
        </thead>
        <tbody>
          <tr v-for="product in data.items" :key="product.id">
            <td>
              <strong>{{ product.title }}</strong>
              <p class="mono wrap">{{ product.slug }}</p>
            </td>
            <td><span class="badge">{{ statusLabel[product.type] }}</span><p class="muted">{{ preparation[product.type] }}</p></td>
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
    <div v-else class="empty"><h2>没有商品</h2><p>点击新建商品，选择教程、数字商品或服务，按发布指引继续。</p></div>
    <div v-if="data.total > data.page_size" class="pagination">
      <button class="btn secondary small" type="button" :disabled="page === 1" @click="goPage(page - 1)">上一页</button>
      <span>{{ page }}</span>
      <button class="btn secondary small" type="button" :disabled="page * data.page_size >= data.total" @click="goPage(page + 1)">下一页</button>
    </div>
  </template>
</template>
