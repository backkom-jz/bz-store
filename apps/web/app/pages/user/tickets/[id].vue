<script setup lang="ts">
import type { Ticket } from '@devstore/shared-types';
import { dateTime, statusLabel } from '@devstore/ui/format';

definePageMeta({ middleware: 'auth' });
useSeoMeta({ title: '工单 · DevStore' });
const route = useRoute();
const api = useApi();
const body = ref('');
const message = ref('');
const sending = ref(false);
const { data, error, status, refresh } = await useAsyncData(
  () => 'private:ticket:' + route.params.id,
  () => api.request<Ticket>('/me/tickets/' + route.params.id),
  { server: false },
);

async function send() {
  sending.value = true;
  message.value = '';
  try {
    data.value = await api.request<Ticket>('/me/tickets/' + route.params.id + '/messages', { method: 'POST', body: { body: body.value.trim() } });
    body.value = '';
  } catch (reason) {
    message.value = reason instanceof Error ? reason.message : '发送失败，请重试';
  } finally {
    sending.value = false;
  }
}

async function accept() {
  sending.value = true;
  message.value = '';
  try {
    data.value = await api.request<Ticket>('/me/tickets/' + route.params.id + '/accept', { method: 'POST' });
  } catch (reason) {
    message.value = reason instanceof Error ? reason.message : '确认失败，请重试';
  } finally {
    sending.value = false;
  }
}
</script>

<template>
  <div class="container">
    <div class="page-head">
      <div class="breadcrumbs"><NuxtLink to="/user/tickets">我的工单</NuxtLink></div>
      <ApiFeedback :pending="status === 'pending'" :error="error" @retry="refresh" />
      <template v-if="data">
        <span class="badge">{{ statusLabel[data.status] || data.status }}</span>
        <h1>{{ data.product_title }}</h1>
      </template>
    </div>
    <ClientOnly>
      <article v-if="data" class="panel stack">
        <div v-for="item in data.messages || []" :key="item.id">
          <p>{{ item.body }}</p>
          <p class="muted">{{ dateTime(item.created_at) }}</p>
        </div>
        <form v-if="data.status === 'open'" class="stack" @submit.prevent="send">
          <label for="reply">留言</label>
          <textarea id="reply" v-model="body" maxlength="4000" required :disabled="sending"></textarea>
          <div class="row">
            <button class="btn" type="submit" :disabled="sending">发送</button>
            <button class="btn secondary" type="button" :disabled="sending" @click="accept">确认交付完成</button>
          </div>
        </form>
        <p v-if="message" class="error" role="alert">{{ message }}</p>
      </article>
    </ClientOnly>
  </div>
</template>
