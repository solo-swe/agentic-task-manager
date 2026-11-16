// Connecting the user's agents from the atm app (T-458): the app's `agents_status` and `connect_agents` commands, the
// same atm-lib code as `atm agents status` and `atm agents install`. They have no REST route: atm serve never writes
// into the user's harness folders, so in the browser the dialog shows the command to run instead.
import { invoke } from '@tauri-apps/api/core';

/** A skill's state where a harness reads the user's skills */
export type SkillState = 'linked' | 'copied' | 'missing' | 'stale' | "not atm's";

/** A harness (a folder agent tools read skills from), as `atm agents status` reports it for the user's folder */
export interface HarnessStatus {
  harness: string;
  label: string;
  /** Whether one of the tools that read it is installed here */
  detected: boolean;
  folder: string;
  skills: { skill: string; path: string; state: SkillState }[];
  /** The tools that read it (T-413: one folder is shared by several) */
  tools?: { tool: string; label: string; detected: boolean }[];
  /** The tools found here that would list atm's skills twice, from this folder and another they read */
  duplicates?: string[];
}

/** What connecting wrote, and what it left alone */
export interface Connected {
  written: { harness: string; skill: string; path: string; how: string; action: string }[];
  refused: { path: string; reason: string }[];
}

/** The harnesses atm knows, with their skills' state */
export function agentsStatus(): Promise<HarnessStatus[]> {
  return invoke<HarnessStatus[]>('agents_status');
}

/** Put this version's skills where `harness` reads the user's: only on the user's click */
export function connectAgents(harness: string): Promise<Connected> {
  return invoke<Connected>('connect_agents', { harness });
}

/** Whether every one of atm's skills is there for this harness, as this version has them */
export function connected(status: HarnessStatus): boolean {
  return status.skills.length > 0 && status.skills.every((s) => s.state === 'linked' || s.state === 'copied');
}
