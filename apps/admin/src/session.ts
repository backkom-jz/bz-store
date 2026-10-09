import { ApiError, createApiClient } from '@devstore/api-client';
import type { PublicUser } from '@devstore/shared-types';
import { reactive } from 'vue';

export const session = reactive<{ user: PublicUser | null; error: string; ready: boolean }>({
  user: null,
  error: '',
  ready: false,
});

export const api = createApiClient({
  baseUrl: '/api/v1',
  browser: true,
  onUnauthorized() {
    session.user = null;
    if (!location.pathname.startsWith('/admin/login')) location.assign('/admin/login');
  },
});

export async function loadSession() {
  try {
    session.user = await api.request<PublicUser>('/users/me');
    session.error = '';
  } catch (error) {
    if (error instanceof ApiError && error.status === 401) session.user = null;
    else session.error = error instanceof Error ? error.message : '暂时无法检查登录状态';
  } finally {
    session.ready = true;
  }
}

export async function login(email: string, password: string) {
  const user = await api.request<PublicUser>('/auth/login', { method: 'POST', body: { email, password } });
  session.user = user;
  session.error = '';
  if (!user.is_admin) throw new Error('这个账号没有管理员权限');
}

export async function logout() {
  await api.request('/auth/logout', { method: 'POST' });
  session.user = null;
  location.assign('/admin/login');
}
