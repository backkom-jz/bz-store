export const channelLabel: Record<string, string> = {
  bank: '银行转账',
  wechat: '微信收款',
  alipay: '支付宝收款',
};

export type StepState = 'done' | 'current' | 'upcoming';

export function checkoutSteps(status: string): StepState[] {
  if (status === 'pending_payment') return ['done', 'current', 'upcoming', 'upcoming'];
  if (status === 'payment_review') return ['done', 'done', 'current', 'upcoming'];
  if (status === 'paid' || status === 'fulfilling') return ['done', 'done', 'done', 'current'];
  if (status === 'completed') return ['done', 'done', 'done', 'done'];
  return ['done', 'upcoming', 'upcoming', 'upcoming'];
}

export function shouldPoll(status: string): boolean {
  return status === 'payment_review' || status === 'paid' || status === 'fulfilling';
}

export function remainingMs(expiresAt: string, now: number): number {
  const end = Date.parse(expiresAt);
  if (Number.isNaN(end)) return 0;
  return end - now;
}

export function formatRemaining(ms: number): string {
  if (ms <= 0) return '已结束';
  const total = Math.floor(ms / 1000);
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const seconds = total % 60;
  if (hours >= 48) return `${Math.floor(hours / 24)} 天 ${hours % 24} 小时`;
  const pad = (value: number) => String(value).padStart(2, '0');
  return `${pad(hours)}:${pad(minutes)}:${pad(seconds)}`;
}

export function yuan(cents: number): string {
  const sign = cents < 0 ? '-' : '';
  const abs = Math.abs(Math.trunc(cents));
  return `${sign}${Math.floor(abs / 100)}.${String(abs % 100).padStart(2, '0')}`;
}

const MAX_RECEIPT = 5 * 1024 * 1024;

export function receiptError(file: { type: string; size: number; name: string }): string | null {
  const type = file.type.toLowerCase();
  const named = !type && /\.(png|jpe?g)$/i.test(file.name);
  if (type !== 'image/png' && type !== 'image/jpeg' && !named) return '请选择 PNG 或 JPEG 图片';
  if (file.size <= 0) return '图片是空文件，请重新选择';
  if (file.size > MAX_RECEIPT) return '请选择不超过 5 MiB 的图片';
  return null;
}

export function instructionsReady(value: { channel?: string; payee?: string; account?: string; amount_cents?: number } | null, amountCents: number): boolean {
  if (!value?.channel || !value.payee?.trim() || !value.account?.trim()) return false;
  return value.amount_cents === undefined || value.amount_cents === amountCents;
}
