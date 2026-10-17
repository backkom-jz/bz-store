<script setup lang="ts">
import type { AdminPayment, OrderDetail } from '@devstore/shared-types';
import { dateTime, money, statusLabel } from '@devstore/ui/format';
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import { approvalCents, canSubmitApproval } from '../review';
import { api } from '../session';

const props = defineProps<{ payment: AdminPayment }>();
const emit = defineEmits<{ close: []; reviewed: [kind: 'approved' | 'rejected'] }>();

const dialog = ref<HTMLDialogElement | null>(null);
const amount = ref('');
const channel = ref(props.payment.payment_channel_expected);
const reference = ref('');
const confirmed = ref(false);
const reason = ref('');
const busy = ref(false);
const error = ref('');
const proofUrl = ref('');
const proofError = ref('');
const settled = ref(props.payment.status === 'pending' ? '' : statusLabel[props.payment.status] || props.payment.status);
const channels: Record<string, string> = { bank: '银行转账', wechat: '微信收款', alipay: '支付宝收款' };

const canApprove = computed(() => canSubmitApproval({
  amount: amount.value,
  channel: channel.value,
  reference: reference.value,
  confirmed: confirmed.value,
  expectedAmountCents: props.payment.amount_cents,
  expectedChannel: props.payment.payment_channel_expected,
}) && !settled.value && !busy.value);

const approvalAmountMismatch = computed(() => {
  try { return approvalCents(amount.value) !== props.payment.amount_cents; } catch { return true; }
});

async function loadProof() {
  proofError.value = '';
  if (proofUrl.value) URL.revokeObjectURL(proofUrl.value);
  proofUrl.value = '';
  try {
    const blob = await api.request<Blob>('/admin/payments/' + props.payment.id + '/proof', { responseType: 'blob' });
    if (!(blob instanceof Blob) || !blob.type.startsWith('image/')) throw new Error('凭证无法预览');
    proofUrl.value = URL.createObjectURL(blob);
  } catch (reason) {
    proofError.value = reason instanceof Error ? reason.message : '无法加载凭证';
  }
}

async function refreshAuthority() {
  try {
    const order = await api.request<OrderDetail>('/admin/orders/' + props.payment.order_id);
    const current = order.payments.find((item) => item.id === props.payment.id);
    if (current && current.status !== 'pending') settled.value = statusLabel[current.status] || current.status;
  } catch {
    error.value = error.value || '暂时无法重新核对审核状态';
  }
}

async function approve() {
  if (!canApprove.value || busy.value) return;
  busy.value = true;
  error.value = '';
  try {
    await api.request('/admin/payments/' + props.payment.id + '/approve', {
      method: 'POST',
      body: {
        actual_amount_cents: approvalCents(amount.value),
        payment_channel: channel.value,
        transaction_reference: reference.value.trim(),
        receipt_confirmed: true,
      },
    });
    emit('reviewed', 'approved');
  } catch (reason) {
    error.value = reason instanceof Error ? reason.message : '审核失败，请重试';
    await refreshAuthority();
  } finally {
    busy.value = false;
  }
}

async function reject() {
  const note = reason.value.trim();
  if (!note || note.length > 1000 || busy.value || settled.value) return;
  busy.value = true;
  error.value = '';
  try {
    await api.request('/admin/payments/' + props.payment.id + '/reject', { method: 'POST', body: { reason: note } });
    emit('reviewed', 'rejected');
  } catch (reason) {
    error.value = reason instanceof Error ? reason.message : '驳回失败，请重试';
    await refreshAuthority();
  } finally {
    busy.value = false;
  }
}

function close() {
  if (busy.value) return;
  emit('close');
}

onMounted(() => {
  dialog.value?.showModal();
  void loadProof();
});
onBeforeUnmount(() => {
  if (proofUrl.value) URL.revokeObjectURL(proofUrl.value);
});
</script>

<template>
  <dialog ref="dialog" class="review-dialog" aria-labelledby="review-title" @cancel.prevent="close">
    <div class="review-layout">
      <div>
        <h2 id="review-title">核对付款凭证</h2>
        <p class="muted">{{ payment.product_title }} · {{ payment.user_email }}</p>
        <p class="mono wrap">{{ payment.order_no }}</p>
        <img v-if="proofUrl" :src="proofUrl" alt="用户提交的付款凭证" class="proof">
        <div v-else-if="proofError" class="error" role="alert">
          <p>{{ proofError }}</p>
          <button class="btn secondary small" type="button" @click="loadProof">重试</button>
        </div>
        <p v-else class="muted" role="status">正在加载凭证…</p>
      </div>
      <div>
        <dl class="facts">
          <dt>订单金额</dt>
          <dd>{{ money(payment.amount_cents) }}</dd>
          <dt>订单渠道</dt>
          <dd>{{ channels[payment.payment_channel_expected] || payment.payment_channel_expected }}</dd>
          <dt>提交时间</dt>
          <dd>{{ dateTime(payment.created_at) }}</dd>
        </dl>
        <p v-if="payment.overdue" class="badge amber">超过 24 小时未审核</p>
        <p v-if="settled" class="notice" role="status">这张凭证已经是「{{ settled }}」，不能再次提交。</p>
        <p v-if="error" class="error" role="alert">{{ error }}</p>
        <div class="review-checklist"><strong>核账步骤</strong><ol><li>在微信等收款账单核对实际到账。</li><li>填写实际金额与交易流水。</li><li>确认金额、渠道与订单一致后批准。</li></ol><p>一分钱验证填写 0.01 元；本系统不会自动扣款。</p></div>
        <form @submit.prevent="approve">
          <div class="field">
            <label for="actual-amount">实际到账金额（元）</label>
            <input id="actual-amount" v-model="amount" inputmode="decimal" autocomplete="off" placeholder="按到账记录填写" :disabled="!!settled || busy">
          </div>
          <div class="field">
            <label for="channel">收款渠道</label>
            <select id="channel" v-model="channel" :disabled="!!settled || busy">
              <option value="">请选择</option>
              <option value="bank">银行转账</option>
              <option value="wechat">微信收款</option>
              <option value="alipay">支付宝收款</option>
            </select>
          </div>
          <div class="field">
            <label for="reference">渠道交易流水号</label>
            <input id="reference" v-model="reference" maxlength="128" autocomplete="off" :disabled="!!settled || busy">
          </div>
          <label class="check" for="confirmed">
            <input id="confirmed" v-model="confirmed" type="checkbox" :disabled="!!settled || busy">
            已核实真实收款记录
          </label>
          <p v-if="amount && approvalAmountMismatch" class="field-error" role="status">实际金额应与订单金额 {{ money(payment.amount_cents) }} 一致，金额不符请核对或驳回。</p>
          <p v-if="channel && channel !== payment.payment_channel_expected" class="field-error" role="status">收款渠道与订单不一致，请核对后处理。</p>
          <button class="btn" type="submit" :disabled="!canApprove">确认到账并开通</button>
        </form>
        <form class="reject" @submit.prevent="reject">
          <div class="field">
            <label for="reason">驳回原因</label>
            <textarea id="reason" v-model="reason" maxlength="1000" :disabled="!!settled || busy"></textarea>
          </div>
          <button class="btn danger" type="submit" :disabled="!reason.trim() || !!settled || busy">驳回凭证</button>
        </form>
        <button class="btn secondary" type="button" :disabled="busy" @click="close">关闭</button>
      </div>
    </div>
  </dialog>
</template>
