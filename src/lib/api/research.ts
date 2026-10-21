import { invoke } from "@tauri-apps/api/core";

export type ResearchState = "research" | "partial" | "verified" | "needs_review";

export interface ResearchEvidence {
  kind: "finding" | "test" | "runtime" | "file" | "note";
  label: string;
  detail: string;
}

export interface ResearchTopic {
  id: string;
  title: string;
  sessionTitle: string;
  timestampMs: number;
  state: ResearchState;
  summary: string;
  conclusions: string[];
  evidence: ResearchEvidence[];
  limitations: string[];
  sourcePath: string;
  sourceNumber: number;
}

export interface ResearchHistory {
  topics: ResearchTopic[];
  scannedFiles: number;
  truncated: boolean;
  warnings: string[];
}

export async function readResearchHistory(workspaceId: string): Promise<ResearchHistory> {
  return invoke<ResearchHistory>("read_research_history", { id: workspaceId });
}
