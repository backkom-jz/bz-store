import { parseCents } from '@devstore/ui/format';

export function canSubmitApproval(input: { amount: string; channel: string; reference: string; confirmed: boolean }): boolean {
  if (!input.confirmed || !input.channel || !input.reference.trim() || input.reference.trim().length > 128) return false;
  try {
    parseCents(input.amount);
    return true;
  } catch {
    return false;
  }
}

export function approvalCents(amount: string): number {
  return parseCents(amount);
}
