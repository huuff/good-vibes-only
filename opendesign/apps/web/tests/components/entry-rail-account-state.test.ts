import { describe, expect, it } from 'vitest';

import {
  resolveEntryRailAccountFooterState,
  requiresAmrReauthentication,
} from '../../src/components/entry-rail-account-state';
import type { WorkspaceContextState } from '../../src/collab/useWorkspaceContext';

const SIGNED_IN_CONTEXT = {
  workspaceId: 'workspace-1',
} as WorkspaceContextState['context'];

describe('resolveEntryRailAccountFooterState', () => {
  it('keeps the resolved account row when a workspace context exists', () => {
    expect(resolveEntryRailAccountFooterState({
      context: SIGNED_IN_CONTEXT,
      loading: false,
      failure: 'unavailable',
    }, true)).toBe('hidden');
  });

  it('hides cloud account UI while the former workspace identity is loading', () => {
    expect(resolveEntryRailAccountFooterState({
      context: null,
      loading: true,
    }, null)).toBe('hidden');
  });

  it.each([true, null] as const)(
    'hides cloud recovery during an outage when former login state is %s',
    (amrLoggedIn) => {
      expect(resolveEntryRailAccountFooterState({
        context: null,
        loading: false,
        failure: 'unavailable',
      }, amrLoggedIn)).toBe('hidden');
    },
  );

  it('does not offer cloud sign-in after an explicit local logout', () => {
    expect(resolveEntryRailAccountFooterState({
      context: null,
      loading: false,
      failure: 'unavailable',
    }, false)).toBe('hidden');
  });

  it('does not offer cloud sign-in when old auth has expired', () => {
    expect(resolveEntryRailAccountFooterState({
      context: null,
      loading: false,
      failure: 'reauth-required',
    }, true, 'reauth_required')).toBe('hidden');
  });

  it('does not keep a stale cached account row above the sign-in card after auth expires', () => {
    expect(resolveEntryRailAccountFooterState({
      context: SIGNED_IN_CONTEXT,
      loading: false,
      failure: 'reauth-required',
    }, true, 'reauth_required')).toBe('hidden');
  });

  it('accepts the next successful null response as authoritative sign-out', () => {
    expect(resolveEntryRailAccountFooterState({
      context: null,
      loading: false,
    }, true)).toBe('hidden');
  });

  it('preserves the legacy unsupported-daemon behavior', () => {
    expect(resolveEntryRailAccountFooterState({
      context: null,
      loading: false,
      failure: 'unsupported',
    }, true)).toBe('hidden');
  });
});

describe('requiresAmrReauthentication', () => {
  it('requires reauthentication when workspace authority detects expiry before status polling', () => {
    expect(requiresAmrReauthentication('authenticated', 'reauth-required')).toBe(true);
  });
});
