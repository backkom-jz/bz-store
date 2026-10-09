export interface ChapterDraft {
  title: string;
  content_md: string;
  is_preview: boolean;
  sort_order: string;
  status: 'draft' | 'published';
}

const slugPattern = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;

export function slugError(value: string): string | null {
  const slug = value.trim();
  if (!slug) return '请填写 slug';
  if (slug.length > 220 || !slugPattern.test(slug)) return 'slug 仅可使用小写字母、数字和单连字符';
  return null;
}

export function centsToYuanInput(cents: number): string {
  if (!Number.isSafeInteger(cents) || cents < 0) return '';
  const whole = Math.floor(cents / 100);
  const fraction = cents % 100;
  if (fraction === 0) return String(whole);
  return `${whole}.${String(fraction).padStart(2, '0')}`;
}

export function parseSortOrder(value: string): number | null {
  if (!/^(0|[1-9]\d{0,5})$/.test(value.trim())) return null;
  return Number(value.trim());
}

export function blankChapter(sortOrder = 0): ChapterDraft {
  return { title: '', content_md: '', is_preview: false, sort_order: String(sortOrder), status: 'draft' };
}
