<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue';
import { RouterLink } from 'vue-router';
import type { AdminPayment, Page } from '@devstore/shared-types';
import { dateTime, money } from '@devstore/ui/format';
import { api } from '../session';

const cards = ref([
  { label: '待审核付款', aria: '查看待审核付款', caption: '核对微信账单后处理', href: '/payments?status=pending', endpoint: '/admin/payments?status=pending&page_size=5', total: null as number | null, failed: false, accent: 'cyan' },
  { label: '待履约订单', aria: '查看待履约订单', caption: '跟进服务交付与用户验收', href: '/orders?status=fulfilling', endpoint: '/admin/orders?status=fulfilling&page_size=1', total: null as number | null, failed: false, accent: 'violet' },
  { label: '全部订单', aria: '查看全部订单', caption: '按订单号或买家邮箱查询', href: '/orders', endpoint: '/admin/orders?page_size=1', total: null as number | null, failed: false, accent: 'blue' },
  { label: '商品总数', aria: '查看全部商品', caption: '管理上架内容与交付文件', href: '/products', endpoint: '/admin/products?page_size=1', total: null as number | null, failed: false, accent: 'green' },
]);
const pending = ref(false);
const payments = ref<AdminPayment[]>([]);
const updated = ref('');
let generation = 0;
async function load() {
  const current = ++generation;
  pending.value = true;
  await Promise.all(cards.value.map(async (card, index) => {
    try {
      const result = await api.request<Page<AdminPayment>>(card.endpoint);
      if (generation !== current) return;
      card.total = result.total; card.failed = false;
      if (index === 0) payments.value = result.items;
    } catch {
      if (generation !== current) return;
      card.total = null; card.failed = true;
      if (index === 0) payments.value = [];
    }
  }));
  if (generation === current) {
    pending.value = false;
    updated.value = new Intl.DateTimeFormat('zh-CN', { hour: '2-digit', minute: '2-digit' }).format(new Date());
  }
}
onMounted(() => void load());
onBeforeUnmount(() => { generation++; });
</script>

<template>
  <header class="page-head spread">
    <div><p class="eyebrow">今日工作</p><h1>运营工作台</h1><p class="muted">先处理收款，再跟进交付。你的日常操作从这里开始。</p></div>
    <button class="btn secondary" type="button" :disabled="pending" @click="load">{{ pending ? '正在刷新…' : '刷新工作台' }}</button>
  </header>
  <div class="stat-grid" :aria-busy="pending">
    <RouterLink v-for="card in cards" :key="card.label" :to="card.href" class="stat-card" :class="card.accent" :aria-label="card.aria">
      <span class="stat-label">{{ card.label }} <span aria-hidden="true">↗</span></span>
      <strong class="stat-number">{{ card.failed ? '暂不可用' : card.total ?? '—' }}</strong>
      <span class="muted">{{ card.failed ? '读取失败，请刷新重试' : card.caption }}</span>
    </RouterLink>
  </div>
  <p class="hint data-note">当前全站记录，包含示例数据。{{ updated ? '最近读取 ' + updated : '正在读取待办…' }}；各项数据分别刷新。</p>
  <div class="workbench-grid">
    <section class="panel work-section">
      <div class="spread section-head"><div><p class="eyebrow">优先处理</p><h2>最近待审核付款</h2></div><RouterLink class="text-link" to="/payments?status=pending">查看全部 →</RouterLink></div>
      <p v-if="cards[0].failed" class="error" role="alert">付款待办读取失败，请刷新重试。</p>
      <p v-else-if="pending && cards[0].total === null" role="status" class="muted">正在读取付款待办…</p>
      <ul v-else-if="payments.length" class="task-list">
        <li v-for="payment in payments" :key="payment.id">
          <div><strong>{{ payment.product_title }}</strong><p class="muted">{{ payment.user_email }}</p><small class="muted">{{ dateTime(payment.created_at) }}</small></div>
          <div class="task-action"><strong>{{ money(payment.amount_cents) }}</strong><RouterLink class="btn small" :to="{ path: '/payments', query: { q: payment.order_no, status: 'pending' } }">去审核</RouterLink></div>
        </li>
      </ul>
      <div v-else class="empty compact"><span class="empty-mark" aria-hidden="true">✓</span><h3>暂时没有待审核付款</h3><p>用户提交付款凭证后，会出现在这里。</p></div>
    </section>
    <section class="panel work-section">
      <p class="eyebrow">日常操作</p><h2>常用入口</h2>
      <div class="shortcut-list"><RouterLink to="/products/new"><strong>＋ 新建商品</strong><span>设置内容、价格与交付方式 →</span></RouterLink><RouterLink to="/tutorials"><strong>编辑教程章节</strong><span>维护预览、付费内容与发布状态 →</span></RouterLink><RouterLink to="/tickets"><strong>跟进服务工单</strong><span>回复买家并核对交付进度 →</span></RouterLink><RouterLink to="/blog/new"><strong>撰写博客</strong><span>保存草稿，准备好后发布 →</span></RouterLink></div>
    </section>
  </div>
  <section class="workflow-guide panel">
    <div><span class="badge green">微信人工收款</span><h2>一笔订单，按这三步处理</h2><p class="muted">当前采用人工收款和核账。微信一分钱验证时，商品价格填写 0.01 元。</p></div>
    <ol class="workflow-steps"><li><b>01</b><div><strong>查看待审核凭证</strong><p>先核对订单、买家和应收金额。</p></div></li><li><b>02</b><div><strong>核实微信实际到账</strong><p>对照账单金额与流水；不能仅凭截图批准。</p></div></li><li><b>03</b><div><strong>批准开通或说明原因驳回</strong><p>教程与文件开通权益，服务进入工单交付。</p></div></li></ol>
  </section>
</template>
