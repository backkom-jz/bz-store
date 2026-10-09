<script setup lang="ts">
import type { AdminPayment, Page } from '@devstore/shared-types';
import { dateTime, money, statusLabel } from '@devstore/ui/format';
import { onMounted, ref, watch } from 'vue';
import { useRoute } from 'vue-router';
import ReviewPaymentDialog from '../components/ReviewPaymentDialog.vue';
import { api } from '../session';

const route = useRoute();
const status = ref('pending');
const q = ref(typeof route.query.q === 'string' ? route.query.q : '');
const page = ref(1);
const data = ref<Page<AdminPayment> | null>(null);
const error = ref('');
const pending = ref(false);
const notice = ref('');
const selected = ref<AdminPayment | null>(null);

async function load() {
  pending.value = true;
  error.value = '';
  const params = new URLSearchParams({ page: String(page.value), page_size: '20' });
  if (status.value) params.set('status', status.value);
  if (q.value.trim()) params.set('q', q.value.trim());
  try {
    data.value = await api.request<Page<AdminPayment>>('/admin/payments?' + params);
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

function onReviewed(kind: 'approved' | 'rejected') {
  selected.value = null;
  notice.value = kind === 'approved' ? '已核实到账，教程已开通。' : '已驳回，用户可以重新提交凭证。';
  void load();
}

watch(status, () => {
  page.value = 1;
  void load();
});
onMounted(() => void load());
</script>

<template>
  <header class="page-head">
    <h1>付款审核</h1>
    <p class="muted">对照凭证和真实到账记录。通过后才会开通教程。</p>
  </header>
  <form class="toolbar" @submit.prevent="search">
    <label class="sr-only" for="pay-q">搜索订单号或邮箱</label>
    <input id="pay-q" v-model="q" placeholder="订单号或邮箱" maxlength="100">
    <label class="sr-only" for="pay-status">审核状态</label>
    <select id="pay-status" v-model="status">
      <option value="pending">待审核</option>
      <option value="approved">已通过</option>
      <option value="rejected">已驳回</option>
      <option value="">全部</option>
    </select>
    <button class="btn secondary" type="submit">搜索</button>
  </form>
  <p v-if="notice" class="notice" role="status">{{ notice }}</p>
  <p v-if="pending" class="muted" role="status">正在加载…</p>
  <div v-else-if="error" class="error" role="alert">
    <p>{{ error }}</p>
    <button class="btn secondary small" type="button" @click="load">重试</button>
  </div>
  <template v-else-if="data">
    <div v-if="data.items.length" class="table-wrap">
      <table>
        <thead>
          <tr>
            <th>订单</th>
            <th>用户</th>
            <th>金额</th>
            <th>状态</th>
            <th>提交时间</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="payment in data.items" :key="payment.id">
            <td>
              <strong>{{ payment.product_title }}</strong>
              <p class="mono wrap">{{ payment.order_no }}</p>
            </td>
            <td class="wrap">{{ payment.user_email }}</td>
            <td>{{ money(payment.amount_cents) }}</td>
            <td>
              <span class="badge" :class="{ purple: payment.status === 'pending', green: payment.status === 'approved' }">{{ statusLabel[payment.status] }}</span>
              <span v-if="payment.overdue" class="badge amber">超过 24 小时</span>
            </td>
            <td>{{ dateTime(payment.created_at) }}</td>
            <td><button class="btn small" type="button" @click="selected = payment">{{ payment.status === 'pending' ? '审核' : '查看' }}</button></td>
          </tr>
        </tbody>
      </table>
    </div>
    <div v-else class="empty"><h2>没有符合条件的凭证</h2><p>新的付款凭证提交后会出现在这里。</p></div>
    <div v-if="data.total > data.page_size" class="pagination">
      <button class="btn secondary small" type="button" :disabled="page === 1" @click="page--; load()">上一页</button>
      <span>{{ page }}</span>
      <button class="btn secondary small" type="button" :disabled="page * data.page_size >= data.total" @click="page++; load()">下一页</button>
    </div>
  </template>
  <ReviewPaymentDialog v-if="selected" :payment="selected" @close="selected = null" @reviewed="onReviewed" />
</template>
