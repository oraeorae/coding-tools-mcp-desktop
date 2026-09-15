import { invoke } from "@tauri-apps/api/core";

export interface LogChunk {
  name: string;
  content: string;
}

export type LogService = "mcp" | "actions";

export interface RequestErrorEntry {
  id: string;
  timestampMs: number;
  service: LogService;
  source: "http" | "rpc" | "health";
  method: string;
  path: string;
  status: number | null;
  code: string;
  message: string;
  requestId: string;
  tool: string;
}

export async function readWorkspaceLogs(
  workspaceId: string,
  service: LogService,
): Promise<LogChunk[]> {
  return invoke<LogChunk[]>("read_workspace_logs", { id: workspaceId, service });
}

export async function readRequestErrors(
  workspaceId: string,
  service: LogService,
): Promise<RequestErrorEntry[]> {
  return invoke<RequestErrorEntry[]>("read_request_errors", { id: workspaceId, service });
}

export async function clearRequestErrors(
  workspaceId: string,
  service: LogService,
): Promise<void> {
  return invoke("clear_request_errors", { id: workspaceId, service });
}
