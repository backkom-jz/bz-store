<script setup lang="ts">
import type { AdminPayment, Page } from '@devstore/shared-types';
import { dateTime, money, statusLabel } from '@devstore/ui/format';
import { ref } from 'vue';
import { RouterLink } from 'vue-router';
import ReviewPaymentDialog from '../components/ReviewPaymentDialog.vue';
import { useAdminList } from '../composables/useAdminList';
const { q, status, page, data, error, pending, load, search, reset, goPage } = useAdminList<AdminPayment>('/admin/payments', 'pending');
const notice = ref('');
const selected = ref<AdminPayment | null>(null);

function onReviewed(kind: 'approved' | 'rejected') {
  selected.value = null;
  notice.value = kind === 'approved' ? '已核实到账，订单已更新：教程与数字商品开通权益，服务订单进入工单交付。' : '已驳回，用户可查看原因后重新提交凭证。';
  void load();
}

</script>

<template>
  <header class="page-head">
    <p class="eyebrow">交易管理</p><h1>付款审核</h1>
    <p class="muted">先核对真实收款账单，再批准开通或说明原因驳回。截图仅作核对线索。</p>
  </header>
  <div class="payment-guidance"><span class="badge green">人工核账</span><p><strong>微信一分钱验证：</strong>订单应收与实际到账都应为 <strong>0.01 元</strong>。填写微信交易流水，核实后再批准；系统不会自动扣款。</p></div>
  <form class="toolbar filter-panel" @submit.prevent="search">
    <div class="filter-field grow"><label for="pay-q">搜索订单号或邮箱</label>
    <input id="pay-q" v-model="q" placeholder="订单号或邮箱" maxlength="100"></div>
    <div class="filter-field"><label for="pay-status">审核状态</label>
    <select id="pay-status" v-model="status" @change="search">
      <option value="pending">待审核</option>
      <option value="approved">已通过</option>
      <option value="rejected">已驳回</option>
      <option value="">全部</option>
    </select></div>
    <button class="btn" type="submit">搜索</button><button class="btn secondary" type="button" @click="reset">重置</button>
  </form>
  <p v-if="notice" class="notice" role="status">{{ notice }}</p>
  <p v-if="pending" class="muted" role="status">正在加载…</p>
  <div v-else-if="error" class="error" role="alert">
    <p>{{ error }}</p>
    <button class="btn secondary small" type="button" @click="load">重试</button>
  </div>
  <template v-else-if="data">
    <div class="list-summary"><span>共 <strong>{{ data.total }}</strong> 条凭证</span><span class="muted">{{ statusLabel[status] || '全部状态' }} · 第 {{ page }} 页</span></div>
    <div v-if="data.items.length" class="table-wrap panel-table">
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
              <p class="mono wrap">{{ payment.order_no }}</p><RouterLink class="text-link" :to="'/orders/' + payment.order_id">查看订单</RouterLink>
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
      <button class="btn secondary small" type="button" :disabled="page === 1" @click="goPage(page - 1)">上一页</button>
      <span>{{ page }}</span>
      <button class="btn secondary small" type="button" :disabled="page * data.page_size >= data.total" @click="goPage(page + 1)">下一页</button>
    </div>
  </template>
  <ReviewPaymentDialog v-if="selected" :payment="selected" @close="selected = null" @reviewed="onReviewed" />
</template>
