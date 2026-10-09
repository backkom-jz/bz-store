<script setup lang="ts">
import type { OrderDetail, PaymentInstructions } from '@devstore/shared-types';
import { dateTime, money, statusLabel } from '@devstore/ui/format';
import { ArrowRight, Check, Clock, Copy, ShieldCheck, Upload } from '@lucide/vue';
import { channelLabel, checkoutSteps, formatRemaining, instructionsReady, receiptError, remainingMs, shouldPoll, yuan } from '~/utils/checkout';

definePageMeta({ middleware: 'auth' });
useSeoMeta({ title: '订单与付款 · DevStore' });

const route = useRoute();
const api = useApi();
const { user } = useSession();
const { data: order, error, status, refresh } = await useAsyncData(
  () => 'private:checkout:' + (user.value?.id || 'none') + ':' + route.params.id,
  () => api.request<OrderDetail>('/orders/' + encodeURIComponent(String(route.params.id))),
  { server: false },
);

const instructions = ref<PaymentInstructions | null>(null);
const instructionsError = ref('');
const instructionsPending = ref(false);
const file = shallowRef<File | null>(null);
const preview = ref('');
const formMessage = ref('');
const pageMessage = ref('');
const uploading = ref(false);
const copied = ref('');
const dragging = ref(false);
const confirmCancel = ref(false);
const cancelling = ref(false);
const now = ref(Date.now());
const proofUrls = ref<Record<string, string>>({});
const proofErrors = ref<Record<string, string>>({});
const proofLoading = new Set<string>();
const fileInput = ref<HTMLInputElement | null>(null);
let clock: ReturnType<typeof setInterval> | undefined;
let poll: ReturnType<typeof setInterval> | undefined;
let copyTimer: ReturnType<typeof setTimeout> | undefined;
let expiryRequested = false;

const steps = computed(() => checkoutSteps(order.value?.status || ''));
const stepNames = ['创建订单', '付款凭证', '人工核实', '开始学习'];
const lastRejection = computed(() => order.value?.payments.filter((payment) => payment.status === 'rejected').at(-1));
const timeLeft = computed(() => order.value?.status === 'pending_payment' ? remainingMs(order.value.expires_at, now.value) : 0);
const expired = computed(() => order.value?.status === 'pending_payment' && timeLeft.value <= 0);
const readyToPay = computed(() => Boolean(order.value && instructionsReady(instructions.value, order.value.amount_cents)));
const amountMismatch = computed(() => Boolean(instructions.value && instructions.value.amount_cents !== order.value?.amount_cents));

watch(() => user.value?.id, (id) => {
  if (!id) void navigateTo({ path: '/login', query: { returnTo: route.fullPath } });
});

watch(() => order.value ? order.value.id + ':' + order.value.status : '', () => {
  void loadInstructions();
});

watch(() => order.value?.payments.map((payment) => payment.id).join(',') ?? '', () => {
  for (const payment of order.value?.payments || []) void loadProof(payment.id);
});

watch(timeLeft, (ms) => {
  if (order.value?.status === 'pending_payment' && ms <= 0 && !expiryRequested) {
    expiryRequested = true;
    void refresh();
  }
});

watch(() => order.value?.expires_at, () => {
  expiryRequested = false;
});

watch(() => order.value?.status, () => syncPoll());

async function loadInstructions() {
  const current = order.value;
  if (!current || current.status !== 'pending_payment' || current.amount_cents === 0) {
    instructions.value = null;
    instructionsError.value = '';
    instructionsPending.value = false;
    return;
  }
  instructionsPending.value = true;
  instructionsError.value = '';
  try {
    const next = await api.request<PaymentInstructions>('/orders/' + current.id + '/payment-instructions');
    if (order.value?.id === current.id) instructions.value = next;
  } catch (reason) {
    if (order.value?.id === current.id) {
      instructions.value = null;
      instructionsError.value = reason instanceof Error ? reason.message : '无法获取付款说明';
    }
  } finally {
    if (order.value?.id === current.id) instructionsPending.value = false;
  }
}

function revokePreview() {
  if (preview.value) URL.revokeObjectURL(preview.value);
  preview.value = '';
}

function takeFile(next: File | undefined) {
  revokePreview();
  file.value = null;
  formMessage.value = '';
  if (!next) return;
  const problem = receiptError(next);
  if (problem) {
    formMessage.value = problem;
    if (fileInput.value) fileInput.value.value = '';
    return;
  }
  file.value = next;
  preview.value = URL.createObjectURL(next);
}

function choose(event: Event) {
  takeFile((event.target as HTMLInputElement).files?.[0]);
}

function leaveDrag(event: DragEvent) {
  const next = event.relatedTarget;
  if (next instanceof Node && (event.currentTarget as Node).contains(next)) return;
  dragging.value = false;
}

function drop(event: DragEvent) {
  dragging.value = false;
  takeFile(event.dataTransfer?.files?.[0]);
}

async function upload() {
  if (!file.value || !order.value || uploading.value || !readyToPay.value || expired.value) return;
  uploading.value = true;
  formMessage.value = '';
  pageMessage.value = '';
  const form = new FormData();
  form.append('file', file.value);
  try {
    await api.request('/orders/' + order.value.id + '/payment-proof', { method: 'POST', body: form });
    file.value = null;
    revokePreview();
    if (fileInput.value) fileInput.value.value = '';
    await refresh();
  } catch (reason) {
    formMessage.value = reason instanceof Error ? reason.message : '上传失败，请重试';
    await refresh();
  } finally {
    uploading.value = false;
  }
}

async function copy(value: string, label: string) {
  try {
    await navigator.clipboard.writeText(value);
    copied.value = label;
    clearTimeout(copyTimer);
    copyTimer = setTimeout(() => {
      if (copied.value === label) copied.value = '';
    }, 2000);
  } catch {
    pageMessage.value = '无法自动复制，请手动选择文字';
  }
}

async function cancelOrder() {
  if (!order.value || cancelling.value) return;
  cancelling.value = true;
  pageMessage.value = '';
  try {
    await api.request('/orders/' + order.value.id + '/cancel', { method: 'POST' });
    confirmCancel.value = false;
    await refresh();
  } catch (reason) {
    pageMessage.value = reason instanceof Error ? reason.message : '取消失败，请重试';
    await refresh();
  } finally {
    cancelling.value = false;
  }
}

function revokeProofs() {
  for (const url of Object.values(proofUrls.value)) URL.revokeObjectURL(url);
  proofUrls.value = {};
  proofErrors.value = {};
}

async function loadProof(id: string) {
  if (!order.value || proofUrls.value[id] || proofErrors.value[id] || proofLoading.has(id)) return;
  const orderId = order.value.id;
  proofLoading.add(id);
  try {
    const blob = await api.request<Blob>('/orders/' + orderId + '/payment-proof/' + id, { responseType: 'blob' });
    if (!(blob instanceof Blob) || !blob.type.startsWith('image/')) throw new Error('凭证无法预览');
    proofUrls.value = { ...proofUrls.value, [id]: URL.createObjectURL(blob) };
  } catch (reason) {
    proofErrors.value = { ...proofErrors.value, [id]: reason instanceof Error ? reason.message : '无法查看凭证' };
  } finally {
    proofLoading.delete(id);
  }
}

function retryProof(id: string) {
  const next = { ...proofErrors.value };
  delete next[id];
  proofErrors.value = next;
  void loadProof(id);
}

function reloadOrder() {
  void refresh();
}

function syncPoll() {
  if (poll) clearInterval(poll);
  poll = undefined;
  if (typeof document === 'undefined' || document.visibilityState !== 'visible') return;
  if (!order.value || !shouldPoll(order.value.status)) return;
  poll = setInterval(() => {
    if (document.visibilityState === 'visible' && order.value && shouldPoll(order.value.status) && status.value !== 'pending') void refresh();
  }, 5000);
}

function onVisibility() {
  if (document.visibilityState === 'hidden') {
    if (poll) clearInterval(poll);
    poll = undefined;
    return;
  }
  if (order.value && (shouldPoll(order.value.status) || order.value.status === 'pending_payment')) void refresh();
  syncPoll();
}

onMounted(() => {
  clock = setInterval(() => {
    now.value = Date.now();
  }, 1000);
  document.addEventListener('visibilitychange', onVisibility);
  void loadInstructions();
  for (const payment of order.value?.payments || []) void loadProof(payment.id);
  syncPoll();
});

onBeforeUnmount(() => {
  if (clock) clearInterval(clock);
  if (poll) clearInterval(poll);
  clearTimeout(copyTimer);
  document.removeEventListener('visibilitychange', onVisibility);
  revokePreview();
  revokeProofs();
});
</script>

<template>
  <div class="container">
    <div class="page-head">
      <div class="breadcrumbs">
        <NuxtLink to="/user/orders">我的订单</NuxtLink>
        <span>/ 订单与付款</span>
      </div>
      <h1>每一步，都清晰可见。</h1>
      <p class="muted">按订单上的金额和收款说明付款。核实到账后，教程会开通到你的账号。</p>
    </div>
    <ClientOnly>
      <ApiFeedback :pending="status === 'pending' && !order" :error="error" @retry="refresh" />
      <div v-if="order" class="checkout-grid">
        <div class="stack">
          <p class="sr-only" aria-live="polite">订单状态：{{ statusLabel[order.status] || order.status }}</p>
          <p v-if="pageMessage" class="error" role="alert">{{ pageMessage }}</p>
          <div class="checkout-steps" role="list" aria-label="付款进度">
            <template v-for="(name, index) in stepNames" :key="name">
              <div class="step" role="listitem" :class="steps[index]" :aria-current="steps[index] === 'current' ? 'step' : undefined">
                <i>{{ index + 1 }}</i>
                <span>{{ name }}</span>
              </div>
              <hr v-if="index < stepNames.length - 1" aria-hidden="true">
            </template>
          </div>

          <section v-if="order.status === 'completed'" class="panel">
            <div class="success-icon"><Check :size="25" /></div>
            <span class="badge green">已完成</span>
            <h2 class="status-title">{{ order.amount_cents === 0 ? '教程已领取' : '教程已开通' }}</h2>
            <p class="muted">可以前往资源中心，开始阅读完整内容。</p>
            <NuxtLink to="/user/library" class="btn">前往我的资源 <ArrowRight :size="16" /></NuxtLink>
          </section>

          <section v-else-if="order.status === 'payment_review'" class="panel">
            <div class="learning-card-icon"><Clock :size="23" /></div>
            <span class="badge purple">付款审核中</span>
            <h2 class="status-title">凭证已提交，等待核实到账</h2>
            <p class="muted">页面会自动更新审核结果。通过后即可阅读；如果需要补充凭证，驳回原因会显示在这里。</p>
            <button class="btn secondary" type="button" :disabled="status === 'pending'" @click="reloadOrder">刷新审核状态</button>
          </section>

          <section v-else-if="order.status === 'paid' || order.status === 'fulfilling'" class="panel">
            <div class="learning-card-icon"><Clock :size="23" /></div>
            <span class="badge purple">{{ statusLabel[order.status] }}</span>
            <h2 class="status-title">到账已确认，正在开通教程</h2>
            <p class="muted">开通完成后，这里会变为可阅读状态。</p>
            <button class="btn secondary" type="button" :disabled="status === 'pending'" @click="reloadOrder">刷新开通状态</button>
          </section>

          <section v-else-if="order.status === 'cancelled'" class="panel">
            <span class="badge">已取消</span>
            <h2 class="status-title">订单已取消</h2>
            <p class="muted">请勿继续向此订单付款。如果已经转出款项，请保留凭证并联系站点管理员，不要对新订单重复支付。</p>
            <NuxtLink to="/products" class="btn secondary">返回商城</NuxtLink>
          </section>

          <section v-else-if="order.status === 'refunded'" class="panel">
            <span class="badge">已退款</span>
            <h2 class="status-title">订单已退款</h2>
            <p class="muted">对应教程访问已关闭。请勿再向此订单付款。</p>
            <NuxtLink to="/user/orders" class="btn secondary">返回我的订单</NuxtLink>
          </section>

          <template v-else-if="order.status === 'pending_payment'">
            <section v-if="expired" class="panel">
              <span class="badge amber">付款时间已结束</span>
              <h2 class="status-title">这笔订单已超过付款期限</h2>
              <p class="muted">请刷新确认是否已关闭。关闭后请勿继续付款；如需购买，回到商城重新下单。若款项已经转出，请保留凭证并联系站点管理员。</p>
              <button class="btn secondary" type="button" :disabled="status === 'pending'" @click="reloadOrder">刷新订单状态</button>
            </section>
            <template v-else>
              <section class="panel">
                <div class="spread">
                  <h2>01 · 核对金额并付款</h2>
                  <span class="badge amber">待付款</span>
                </div>
                <div v-if="lastRejection" role="status" class="error">
                  <strong>上次凭证未通过审核</strong>
                  <p>{{ lastRejection.review_note || '请根据审核说明重新提交凭证。' }}</p>
                </div>
                <p v-if="instructionsPending" class="muted" role="status">正在获取收款说明…</p>
                <div v-else-if="instructionsError" class="error" role="alert">
                  <p>{{ instructionsError }}</p>
                  <button class="btn secondary small" type="button" @click="loadInstructions">重试获取收款说明</button>
                </div>
                <div v-else-if="amountMismatch" class="error" role="alert">
                  <p>收款说明与订单金额不一致，已暂停提交。请刷新页面；若仍然不一致，请联系站点管理员。</p>
                  <button class="btn secondary small" type="button" @click="reloadOrder">刷新订单</button>
                </div>
                <dl v-else-if="instructions && readyToPay" class="payment-details">
                  <dt>付款方式</dt>
                  <dd>{{ channelLabel[instructions.channel] || '收款账户' }}</dd>
                  <dt>收款人</dt>
                  <dd>{{ instructions.payee }}</dd>
                  <dt>收款账号</dt>
                  <dd class="detail-copy">
                    <span class="mono">{{ instructions.account }}</span>
                    <button class="copy-button" type="button" @click="copy(instructions.account, 'account')">
                      <Copy :size="12" />{{ copied === 'account' ? '已复制' : '复制' }}
                    </button>
                  </dd>
                  <dt>实付金额</dt>
                  <dd class="detail-copy">
                    <span class="price pay-amount">{{ money(order.amount_cents) }}</span>
                    <button class="copy-button" type="button" @click="copy(yuan(order.amount_cents), 'amount')">
                      <Copy :size="12" />{{ copied === 'amount' ? '已复制' : '复制金额' }}
                    </button>
                  </dd>
                  <dt>订单备注</dt>
                  <dd class="detail-copy">
                    <span class="mono">{{ order.order_no }}</span>
                    <button class="copy-button" type="button" @click="copy(order.order_no, 'order')">
                      <Copy :size="12" />{{ copied === 'order' ? '已复制' : '复制' }}
                    </button>
                  </dd>
                </dl>
                <p v-if="instructions && readyToPay" class="notice">{{ instructions.instructions }}</p>
                <p class="muted fine-print">转账时请备注订单号，并按上方金额原样支付。付款有效期至 {{ dateTime(order.expires_at) }}。</p>
              </section>

              <section class="panel">
                <h2>02 · 提交付款凭证</h2>
                <form @submit.prevent="upload">
                  <div class="upload-zone" :class="{ hot: dragging }" @dragover.prevent="dragging = true" @dragleave.prevent="leaveDrag" @drop.prevent="drop">
                    <div class="row">
                      <Upload :size="21" />
                      <div>
                        <label for="receipt">选择付款凭证</label>
                        <p class="muted fine-print">PNG 或 JPEG，不超过 5 MiB。请保留金额、时间和流水信息。也可以把图片拖到这里。</p>
                      </div>
                    </div>
                    <input id="receipt" ref="fileInput" type="file" accept="image/png,image/jpeg" :disabled="uploading" @change="choose">
                    <img v-if="preview" :src="preview" alt="待上传凭证预览" class="upload-preview">
                  </div>
                  <p v-if="formMessage" class="error" role="alert">{{ formMessage }}</p>
                  <button class="btn" type="submit" :disabled="!file || uploading || !readyToPay">
                    {{ uploading ? '正在扫描并提交…' : '提交付款凭证' }}
                    <ArrowRight :size="16" />
                  </button>
                </form>
              </section>
            </template>
          </template>

          <section v-if="order.payments.length" class="panel">
            <h2>审核记录</h2>
            <div v-for="payment in order.payments" :key="payment.id" class="proof-card">
              <div class="spread">
                <span class="badge" :class="{ green: payment.status === 'approved', purple: payment.status === 'pending' }">{{ statusLabel[payment.status] }}</span>
                <span class="muted fine-print">{{ dateTime(payment.created_at) }}</span>
              </div>
              <p v-if="payment.review_note">{{ payment.review_note }}</p>
              <img v-if="proofUrls[payment.id]" :src="proofUrls[payment.id]" alt="已提交的付款凭证" class="proof-preview">
              <div v-else-if="proofErrors[payment.id]" class="error" role="alert">
                <p>{{ proofErrors[payment.id] }}</p>
                <button class="btn secondary small" type="button" @click="retryProof(payment.id)">重试</button>
              </div>
              <p v-else class="muted fine-print" role="status">正在加载凭证…</p>
            </div>
          </section>
        </div>

        <aside class="panel checkout-summary">
          <span class="eyebrow">ORDER SUMMARY</span>
          <h2>{{ order.item.product_title }}</h2>
          <p class="muted fine-print">图文教程 · 1 件</p>
          <div class="divider" />
          <div class="spread">
            <span class="muted">订单金额</span>
            <span class="price">{{ money(order.amount_cents) }}</span>
          </div>
          <div class="divider" />
          <p class="fine-label">订单编号</p>
          <p class="mono wrap order-no">{{ order.order_no }}</p>
          <button class="copy-button" type="button" @click="copy(order.order_no, 'summary')">
            <Copy :size="12" />{{ copied === 'summary' ? '已复制' : '复制订单号' }}
          </button>
          <template v-if="order.status === 'pending_payment'">
            <div class="divider" />
            <p class="fine-label">剩余付款时间</p>
            <p class="countdown-value" role="timer" :aria-label="'剩余付款时间 ' + formatRemaining(timeLeft)">{{ formatRemaining(timeLeft) }}</p>
            <div class="divider" />
            <div v-if="confirmCancel" role="group" aria-label="确认取消订单">
              <p>取消后请勿再向此订单付款。</p>
              <div class="row">
                <button class="btn danger small" type="button" :disabled="cancelling" @click="cancelOrder">{{ cancelling ? '正在取消…' : '确认取消' }}</button>
                <button class="btn secondary small" type="button" :disabled="cancelling" @click="confirmCancel = false">继续付款</button>
              </div>
            </div>
            <button v-else class="btn secondary" type="button" @click="confirmCancel = true">取消订单</button>
          </template>
          <div class="divider" />
          <p class="row muted fine-print"><ShieldCheck :size="17" /> 权益与订单关联，开通记录可查</p>
          <NuxtLink class="text-link fine-print" to="/user/orders">返回我的订单</NuxtLink>
        </aside>
      </div>
    </ClientOnly>
  </div>
</template>
