<script setup lang="ts">
import type { OrderDetail } from '@devstore/shared-types';
import { dateTime, money, parseCents, statusLabel } from '@devstore/ui/format';
import { onMounted, ref } from 'vue';
import { RouterLink, useRoute } from 'vue-router';
import { api } from '../session';

const route = useRoute();
const order = ref<OrderDetail | null>(null);
const error = ref('');
const notice = ref('');
const pending = ref(false);
const refunding = ref(false);
const amount = ref('');
const reference = ref('');
const note = ref('');

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

async function refund() {
  if (!order.value || refunding.value) return;
  refunding.value = true;
  error.value = '';
  notice.value = '';
  try {
    order.value = await api.request<OrderDetail>('/admin/orders/' + order.value.id + '/refund', {
      method: 'POST',
      body: { amount_cents: parseCents(amount.value.trim()), reference_no: reference.value.trim(), note: note.value.trim() },
    });
    notice.value = '退款已登记，权益已收回。';
  } catch (reason) {
    error.value = reason instanceof Error ? reason.message : '退款登记失败，请重试';
  } finally {
    refunding.value = false;
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
      <dt>商品类型</dt><dd>{{ statusLabel[order.item.product_type] || order.item.product_type }}</dd>
      <dt>订单金额</dt>
      <dd>{{ money(order.amount_cents) }}</dd>
      <dt>创建时间</dt>
      <dd>{{ dateTime(order.created_at) }}</dd>
      <dt>付款期限</dt>
      <dd>{{ dateTime(order.expires_at) }}</dd>
    </dl>
    <p v-if="order.status === 'pending_payment'" class="notice">等待用户转账并提交凭证；收到截图后仍需核对真实账单。</p><p v-else-if="order.status === 'payment_review'" class="notice">付款凭证已提交，请进入付款审核核对实际到账。</p><p v-else-if="order.status === 'fulfilling'" class="notice">收款已确认，请在服务工单中跟进交付与用户验收。</p>
    <h2>付款凭证与审核记录</h2>
    <p v-if="!order.payments.length" class="muted">还没有提交凭证。</p>
    <ul v-else class="plain">
      <li v-for="payment in order.payments" :key="payment.id">
        <span class="badge">{{ statusLabel[payment.status] }}</span>
        <span class="muted">{{ dateTime(payment.created_at) }}</span>
        <p v-if="payment.review_note">{{ payment.review_note }}</p>
      </li>
    </ul>
    <RouterLink v-if="order.status === 'payment_review'" class="btn" :to="{ path: '/payments', query: { q: order.order_no, status: 'pending' } }">去审核这笔订单</RouterLink>
    <form v-if="order.status === 'completed' || order.status === 'fulfilling'" class="stack" style="margin-top:24px" @submit.prevent="refund">
      <h2>登记整单退款</h2>
      <p class="muted">金额必须与订单一致。登记后权益立即失效，未完成工单会关闭。这里不发起转账。</p>
      <div class="field"><label for="refund-amount">退款金额（元）</label><input id="refund-amount" v-model="amount" required :disabled="refunding"></div>
      <div class="field"><label for="refund-reference">退款流水号</label><input id="refund-reference" v-model="reference" maxlength="128" required :disabled="refunding"></div>
      <div class="field"><label for="refund-note">备注</label><textarea id="refund-note" v-model="note" maxlength="2000" required :disabled="refunding"></textarea></div>
      <button class="btn" type="submit" :disabled="refunding">{{ refunding ? '正在登记…' : '登记退款' }}</button>
    </form>
    <p v-if="notice" class="notice" role="status">{{ notice }}</p>
  </article>
</template>
