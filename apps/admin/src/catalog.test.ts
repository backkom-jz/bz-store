import { describe, expect, it } from 'vitest';
import { blankChapter, centsToYuanInput, parseSortOrder, slugError } from './catalog';

describe('catalog editor', () => {
  it('keeps a new chapter private and unpublished', () => {
    expect(blankChapter(2)).toEqual({
      title: '',
      content_md: '',
      is_preview: false,
      sort_order: '2',
      status: 'draft',
    });
  });

  it('turns stored cents into an exact yuan field', () => {
    expect(centsToYuanInput(9900)).toBe('99');
    expect(centsToYuanInput(10)).toBe('0.10');
    expect(centsToYuanInput(101)).toBe('1.01');
    expect(centsToYuanInput(-1)).toBe('');
  });

  it('rejects slugs and sort orders the server will refuse', () => {
    expect(slugError('rust-axum-starter')).toBeNull();
    expect(slugError('Rust_API')).toContain('slug');
    expect(slugError('-bad')).toContain('slug');
    expect(parseSortOrder('0')).toBe(0);
    expect(parseSortOrder('12')).toBe(12);
    expect(parseSortOrder('1.5')).toBeNull();
    expect(parseSortOrder('-1')).toBeNull();
  });
});
