import { parseCents } from '@devstore/ui/format';

export function canSubmitApproval(input: { amount: string; channel: string; reference: string; confirmed: boolean; expectedAmountCents?: number; expectedChannel?: string }): boolean {
  if (!input.confirmed || !input.channel || !input.reference.trim() || input.reference.trim().length > 128) return false;
  try {
    const cents = parseCents(input.amount);
    if (input.expectedAmountCents !== undefined && cents !== input.expectedAmountCents) return false;
    if (input.expectedChannel !== undefined && input.channel !== input.expectedChannel) return false;
    return true;
  } catch {
    return false;
  }
}

export function approvalCents(amount: string): number {
  return parseCents(amount);
}
