<script setup lang="ts">
import type { OrderDetail } from '@devstore/shared-types';
import { dateTime, money, statusLabel } from '@devstore/ui/format';
import { onMounted, ref } from 'vue';
import { RouterLink, useRoute } from 'vue-router';
import { api } from '../session';

const route = useRoute();
const order = ref<OrderDetail | null>(null);
const error = ref('');
const pending = ref(false);

async function load() {
  pending.value = true;
  error.value = '';
  try {
    order.value = await api.request<OrderDetail>('/admin/orders/' + route.params.id);
  } catch (reason) {
    error.value = reason instanceof Error ? reason.message : '加载失败，请重试';
  } finally {
    pending.value = false;
  }
}

onMounted(() => void load());
</script>

<template>
  <p><RouterLink class="text-link" to="/orders">返回订单列表</RouterLink></p>
  <p v-if="pending" class="muted" role="status">正在加载…</p>
  <div v-else-if="error" class="error" role="alert">
    <p>{{ error }}</p>
    <button class="btn secondary small" type="button" @click="load">重试</button>
  </div>
  <article v-else-if="order" class="panel">
    <span class="badge">{{ statusLabel[order.status] || order.status }}</span>
    <h1>{{ order.item.product_title }}</h1>
    <p class="mono wrap">{{ order.order_no }}</p>
    <dl class="facts">
      <dt>金额</dt>
      <dd>{{ money(order.amount_cents) }}</dd>
      <dt>创建时间</dt>
      <dd>{{ dateTime(order.created_at) }}</dd>
      <dt>付款期限</dt>
      <dd>{{ dateTime(order.expires_at) }}</dd>
    </dl>
    <h2>付款凭证</h2>
    <p v-if="!order.payments.length" class="muted">还没有提交凭证。</p>
    <ul v-else class="plain">
      <li v-for="payment in order.payments" :key="payment.id">
        <span class="badge">{{ statusLabel[payment.status] }}</span>
        <span class="muted">{{ dateTime(payment.created_at) }}</span>
        <p v-if="payment.review_note">{{ payment.review_note }}</p>
      </li>
    </ul>
    <RouterLink v-if="order.status === 'payment_review'" class="btn" :to="{ path: '/payments', query: { q: order.order_no } }">去审核这笔订单</RouterLink>
  </article>
</template>
