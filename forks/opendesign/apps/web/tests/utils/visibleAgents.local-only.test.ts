import { describe, expect, it } from 'vitest';

import { isVisibleLocalCliAgent } from '../../src/utils/visibleAgents';

describe('local-only agent visibility', () => {
  it('hides the Open Design Cloud runtime', () => {
    expect(isVisibleLocalCliAgent({ id: 'amr' })).toBe(false);
  });

  it('keeps local coding agents visible', () => {
    expect(isVisibleLocalCliAgent({ id: 'codex' })).toBe(true);
    expect(isVisibleLocalCliAgent({ id: 'claude' })).toBe(true);
  });
});
