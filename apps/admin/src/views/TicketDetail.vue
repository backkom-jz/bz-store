<script setup lang="ts">
import type { Ticket } from '@devstore/shared-types';
import { dateTime, statusLabel } from '@devstore/ui/format';
import { onMounted, ref } from 'vue';
import { RouterLink, useRoute } from 'vue-router';
import { api } from '../session';

const route = useRoute();
const ticket = ref<Ticket | null>(null);
const body = ref('');
const error = ref('');
const pending = ref(false);
const sending = ref(false);

async function load() {
  pending.value = true;
  error.value = '';
  try {
    ticket.value = await api.request<Ticket>('/admin/tickets/' + route.params.id);
  } catch (reason) {
    error.value = reason instanceof Error ? reason.message : '加载失败，请重试';
  } finally {
    pending.value = false;
  }
}

async function send() {
  if (!ticket.value || sending.value) return;
  sending.value = true;
  error.value = '';
  try {
    ticket.value = await api.request<Ticket>('/admin/tickets/' + ticket.value.id + '/messages', {
      method: 'POST',
      body: { body: body.value.trim() },
    });
    body.value = '';
  } catch (reason) {
    error.value = reason instanceof Error ? reason.message : '发送失败，请重试';
  } finally {
    sending.value = false;
  }
}

onMounted(() => void load());
</script>

<template>
  <p><RouterLink class="text-link" to="/tickets">返回工单</RouterLink></p>
  <p v-if="pending" class="muted" role="status">正在加载…</p>
  <div v-else-if="error && !ticket" class="error" role="alert">
    <p>{{ error }}</p>
    <button class="btn secondary small" type="button" @click="load">重试</button>
  </div>
  <article v-else-if="ticket" class="panel">
    <span class="badge">{{ statusLabel[ticket.status] || ticket.status }}</span>
    <h1>{{ ticket.product_title }}</h1>
    <p class="muted">{{ ticket.user_email }} · {{ dateTime(ticket.created_at) }}</p>
    <ul class="plain">
      <li v-for="message in ticket.messages || []" :key="message.id">
        <p>{{ message.body }}</p>
        <p class="muted">{{ dateTime(message.created_at) }}</p>
      </li>
    </ul>
    <form v-if="ticket.status === 'open'" class="stack" @submit.prevent="send">
      <label for="reply">回复</label>
      <textarea id="reply" v-model="body" maxlength="4000" required :disabled="sending"></textarea>
      <button class="btn" type="submit" :disabled="sending">{{ sending ? '正在发送…' : '发送' }}</button>
    </form>
    <p v-if="error" class="error" role="alert">{{ error }}</p>
  </article>
</template>
