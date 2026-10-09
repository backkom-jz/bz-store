<script setup lang="ts">
import type { OrderStatus, Page } from '@devstore/shared-types';
import { dateTime, money, statusLabel } from '@devstore/ui/format';
import { onMounted, ref, watch } from 'vue';
import { RouterLink } from 'vue-router';
import { api } from '../session';

interface AdminOrderRow {
  id: string;
  order_no: string;
  amount_cents: number;
  status: OrderStatus;
  created_at: string;
  user_email?: string;
  item: { product_title: string };
}

const status = ref('');
const page = ref(1);
const data = ref<Page<AdminOrderRow> | null>(null);
const error = ref('');
const pending = ref(false);

async function load() {
  pending.value = true;
  error.value = '';
  const params = new URLSearchParams({ page: String(page.value), page_size: '20' });
  if (status.value) params.set('status', status.value);
  try {
    data.value = await api.request<Page<AdminOrderRow>>('/admin/orders?' + params);
  } catch (reason) {
    error.value = reason instanceof Error ? reason.message : '加载失败，请重试';
  } finally {
    pending.value = false;
  }
}

watch(status, () => {
  page.value = 1;
  void load();
});
onMounted(() => void load());
</script>

<template>
  <header class="page-head">
    <h1>订单</h1>
    <p class="muted">查看下单、付款和开通结果。金额来自下单时的商品快照。</p>
  </header>
  <div class="toolbar">
    <label class="sr-only" for="order-status">订单状态</label>
    <select id="order-status" v-model="status">
      <option value="">全部状态</option>
      <option value="pending_payment">待付款</option>
      <option value="payment_review">付款审核中</option>
      <option value="completed">已完成</option>
      <option value="cancelled">已取消</option>
      <option value="refunded">已退款</option>
    </select>
  </div>
  <p v-if="pending" class="muted" role="status">正在加载…</p>
  <div v-else-if="error" class="error" role="alert">
    <p>{{ error }}</p>
    <button class="btn secondary small" type="button" @click="load">重试</button>
  </div>
  <template v-else-if="data">
    <div v-if="data.items.length" class="table-wrap">
      <table>
        <thead>
          <tr><th>订单</th><th>用户</th><th>金额</th><th>状态</th><th>时间</th><th></th></tr>
        </thead>
        <tbody>
          <tr v-for="order in data.items" :key="order.id">
            <td><strong>{{ order.item.product_title }}</strong><p class="mono wrap">{{ order.order_no }}</p></td>
            <td class="wrap">{{ order.user_email }}</td>
            <td>{{ money(order.amount_cents) }}</td>
            <td><span class="badge">{{ statusLabel[order.status] || order.status }}</span></td>
            <td>{{ dateTime(order.created_at) }}</td>
            <td><RouterLink class="text-link" :to="'/orders/' + order.id">详情</RouterLink></td>
          </tr>
        </tbody>
      </table>
    </div>
    <div v-else class="empty"><h2>没有订单</h2></div>
    <div v-if="data.total > data.page_size" class="pagination">
      <button class="btn secondary small" type="button" :disabled="page === 1" @click="page--; load()">上一页</button>
      <span>{{ page }}</span>
      <button class="btn secondary small" type="button" :disabled="page * data.page_size >= data.total" @click="page++; load()">下一页</button>
    </div>
  </template>
</template>
