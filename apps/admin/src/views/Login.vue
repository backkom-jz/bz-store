<script setup lang="ts">
import { safeReturnTo } from '@devstore/api-client';
import { ref } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { login, session } from '../session';

const route = useRoute();
const router = useRouter();
const email = ref('');
const password = ref('');
const totpCode = ref('');
const pending = ref(false);
const error = ref('');

async function submit() {
  pending.value = true;
  error.value = '';
  try {
    await login(email.value, password.value, totpCode.value);
    password.value = '';
    await router.replace(safeReturnTo(route.query.returnTo));
  } catch (reason) {
    error.value = reason instanceof Error ? reason.message : '登录失败，请重试';
  } finally {
    pending.value = false;
  }
}
</script>

<template>
  <main id="main" class="login-card panel">
    <p class="eyebrow">ADMIN</p>
    <h1>欢迎回到运营工作台。</h1>
    <p class="muted">管理商品、处理付款、跟进服务交付。请使用管理员账号和动态验证码登录。</p>
    <p v-if="session.error" class="error" role="alert">{{ session.error }}</p>
    <p v-if="session.user && !session.user.is_admin" class="error" role="alert">当前登录的不是管理员。可以换一个管理员账号，或返回商城。</p>
    <form @submit.prevent="submit">
      <div class="field">
        <label for="email">邮箱</label>
        <input id="email" v-model="email" type="email" autocomplete="username" required maxlength="255">
      </div>
      <div class="field">
        <label for="password">密码</label>
        <input id="password" v-model="password" type="password" autocomplete="current-password" required minlength="12" maxlength="128">
      </div>
      <div class="field">
        <label for="totp">动态验证码或恢复码</label>
        <input id="totp" v-model="totpCode" autocomplete="one-time-code" required minlength="6" maxlength="64">
      </div>
      <p v-if="error" class="error" role="alert">{{ error }}</p>
      <button class="btn" type="submit" :disabled="pending">{{ pending ? '正在登录…' : '登录后台' }}</button>
    </form>
    <p><a href="/">返回商城</a></p>
  </main>
</template>
