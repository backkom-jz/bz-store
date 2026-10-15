<script setup lang="ts">
import type { Page, Ticket } from '@devstore/shared-types';
import { dateTime, statusLabel } from '@devstore/ui/format';
import { onMounted, ref } from 'vue';
import { RouterLink } from 'vue-router';
import { api } from '../session';

const page = ref(1);
const data = ref<Page<Ticket> | null>(null);
const error = ref('');
const pending = ref(false);

async function load() {
  pending.value = true;
  error.value = '';
  try {
    data.value = await api.request<Page<Ticket>>('/admin/tickets?page=' + page.value);
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
    <h1>工单</h1>
  </div>
  <p v-if="pending" class="muted" role="status">正在加载…</p>
  <div v-else-if="error" class="error" role="alert">
    <p>{{ error }}</p>
    <button class="btn secondary small" type="button" @click="load">重试</button>
  </div>
  <table v-else-if="data" class="table">
    <thead>
      <tr><th>商品</th><th>买家</th><th>状态</th><th>创建时间</th><th></th></tr>
    </thead>
    <tbody>
      <tr v-if="!data.items.length"><td colspan="5" class="muted">还没有服务工单。</td></tr>
      <tr v-for="ticket in data.items" :key="ticket.id">
        <td>{{ ticket.product_title }}</td>
        <td>{{ ticket.user_email }}</td>
        <td><span class="badge">{{ statusLabel[ticket.status] || ticket.status }}</span></td>
        <td>{{ dateTime(ticket.created_at) }}</td>
        <td><RouterLink class="text-link" :to="'/tickets/' + ticket.id">查看</RouterLink></td>
      </tr>
    </tbody>
  </table>
</template>
