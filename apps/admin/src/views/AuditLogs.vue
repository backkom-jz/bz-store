<script setup lang="ts">
import type { AuditLog, Page } from '@devstore/shared-types';
import { dateTime } from '@devstore/ui/format';
import { onMounted, ref } from 'vue';
import { api } from '../session';

const page = ref(1);
const data = ref<Page<AuditLog> | null>(null);
const error = ref('');
const pending = ref(false);
const actionLabel: Record<string, string> = {
  'payment.approve': '核实到账',
  'payment.reject': '驳回凭证',
  'payment_config.update': '更新收款配置',
  'product.create': '创建商品',
  'product.update': '修改商品',
  'tutorial.create': '创建教程',
  'tutorial.update': '修改教程',
  'chapter.create': '添加章节',
  'chapter.update': '修改章节',
  'category.create': '创建分类',
};

function summary(detail: Record<string, unknown>) {
  const text = JSON.stringify(detail);
  return text.length > 240 ? text.slice(0, 240) + '…' : text;
}

async function load() {
  pending.value = true;
  error.value = '';
  try {
    data.value = await api.request<Page<AuditLog>>('/admin/audit-logs?page=' + page.value + '&page_size=20');
  } catch (reason) {
    error.value = reason instanceof Error ? reason.message : '加载失败，请重试';
  } finally {
    pending.value = false;
  }
}

onMounted(() => void load());
</script>

<template>
  <header class="page-head">
    <h1>审计</h1>
    <p class="muted">记录管理员做了什么。这里只显示操作摘要，不包含凭证图片或密码。</p>
  </header>
  <p v-if="pending" class="muted" role="status">正在加载…</p>
  <div v-else-if="error" class="error" role="alert">
    <p>{{ error }}</p>
    <button class="btn secondary small" type="button" @click="load">重试</button>
  </div>
  <template v-else-if="data">
    <div v-if="data.items.length" class="table-wrap">
      <table>
        <thead>
          <tr><th>时间</th><th>操作者</th><th>动作</th><th>摘要</th></tr>
        </thead>
        <tbody>
          <tr v-for="log in data.items" :key="log.id">
            <td>{{ dateTime(log.created_at) }}</td>
            <td class="wrap">{{ log.actor_email || '未知' }}</td>
            <td>{{ actionLabel[log.action] || log.action }}</td>
            <td class="mono wrap">{{ summary(log.detail) }}</td>
          </tr>
        </tbody>
      </table>
    </div>
    <div v-else class="empty"><h2>还没有审计记录</h2></div>
    <div v-if="data.total > data.page_size" class="pagination">
      <button class="btn secondary small" type="button" :disabled="page === 1" @click="page--; load()">上一页</button>
      <span>{{ page }}</span>
      <button class="btn secondary small" type="button" :disabled="page * data.page_size >= data.total" @click="page++; load()">下一页</button>
    </div>
  </template>
</template>
