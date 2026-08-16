import type { AgentInfo } from '../types';

// AMR is the Open Design Cloud runtime. This fork supports local CLIs and
// BYOK only, so never expose AMR through Settings or model switchers.
const HIDDEN_LOCAL_CLI_AGENT_IDS = new Set(['amr', 'byok-opencode']);

export function isVisibleLocalCliAgent(agent: Pick<AgentInfo, 'id'>): boolean {
  return !HIDDEN_LOCAL_CLI_AGENT_IDS.has(agent.id);
}

export function deepSeekHarnessNeedsSetup(agent: AgentInfo): boolean {
  return (
    agent.id === 'deepseek-harness' &&
    !agent.available &&
    Boolean(agent.path) &&
    Boolean(
      agent.diagnostics?.some(
        (diagnostic) => diagnostic.reason === 'runtime-profile-incompatible',
      ),
    )
  );
}
