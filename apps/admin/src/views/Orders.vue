<script setup lang="ts">
import type { OrderStatus } from '@devstore/shared-types';
import { dateTime, money, statusLabel } from '@devstore/ui/format';
import { RouterLink } from 'vue-router';
import { useAdminList } from '../composables/useAdminList';
interface AdminOrderRow {
  id: string; order_no: string; amount_cents: number; status: OrderStatus;
  created_at: string; user_email?: string; item: { product_title: string; product_type: string };
}
const { q, status, page, data, error, pending, load, search, reset, goPage } = useAdminList<AdminOrderRow>('/admin/orders');
</script>

<template>
  <header class="page-head spread"><div><p class="eyebrow">交易管理</p><h1>订单</h1><p class="muted">从下单到交付，按订单号或买家邮箱快速定位。金额以历史订单为准。</p></div><RouterLink class="btn secondary" to="/payments?status=pending">处理待审核付款 →</RouterLink></header>
  <form class="toolbar filter-panel" @submit.prevent="search">
    <div class="filter-field grow"><label for="order-q">订单号 / 买家邮箱 / 商品名称</label><input id="order-q" v-model="q" placeholder="输入关键词查询订单" maxlength="100"></div>
    <div class="filter-field"><label for="order-status">订单状态</label><select id="order-status" v-model="status" @change="search"><option value="">全部状态</option><option value="pending_payment">待付款</option><option value="payment_review">付款审核中</option><option value="paid">已付款</option><option value="fulfilling">交付中</option><option value="completed">已完成</option><option value="cancelled">已取消</option><option value="refunded">已退款</option></select></div>
    <button class="btn" type="submit">查询订单</button><button class="btn secondary" type="button" @click="reset">重置</button>
  </form>
  <p v-if="pending" class="muted" role="status">正在查询订单…</p>
  <div v-else-if="error" class="error" role="alert"><p>{{ error }}</p><button class="btn secondary small" type="button" @click="load">重试</button></div>
  <template v-else-if="data">
    <div class="list-summary"><span>共 <strong>{{ data.total }}</strong> 笔订单</span><span class="muted">{{ statusLabel[status] || '全部状态' }} · 第 {{ page }} 页</span></div>
    <div v-if="data.items.length" class="table-wrap panel-table"><table><thead><tr><th>商品与订单</th><th>买家</th><th>订单金额</th><th>状态</th><th>下单时间</th><th>操作</th></tr></thead><tbody>
      <tr v-for="order in data.items" :key="order.id"><td><strong>{{ order.item.product_title }}</strong><p class="mono wrap">{{ order.order_no }}</p><span class="muted">{{ statusLabel[order.item.product_type] }}</span></td><td class="wrap">{{ order.user_email }}</td><td><strong>{{ money(order.amount_cents) }}</strong></td><td><span class="badge" :class="{ green: order.status === 'completed', purple: order.status === 'payment_review', amber: order.status === 'fulfilling' }">{{ statusLabel[order.status] || order.status }}</span></td><td>{{ dateTime(order.created_at) }}</td><td><div class="row-actions"><RouterLink class="text-link" :to="'/orders/' + order.id">查看详情</RouterLink><RouterLink v-if="order.status === 'payment_review'" class="btn small" :to="{ path: '/payments', query: { q: order.order_no, status: 'pending' } }">去审核</RouterLink><RouterLink v-if="order.status === 'fulfilling'" class="text-link" to="/tickets">跟进工单</RouterLink></div></td></tr>
    </tbody></table></div>
    <div v-else class="empty panel"><h2>没有符合条件的订单</h2><p>试试其他订单号、邮箱或状态，也可以重置筛选查看全部订单。</p><button class="btn secondary" type="button" @click="reset">清除筛选</button></div>
    <div v-if="data.total > data.page_size" class="pagination"><button class="btn secondary small" type="button" :disabled="page === 1" @click="goPage(page - 1)">上一页</button><span>第 {{ page }} / {{ Math.ceil(data.total / data.page_size) }} 页</span><button class="btn secondary small" type="button" :disabled="page * data.page_size >= data.total" @click="goPage(page + 1)">下一页</button></div>
  </template>
</template>
