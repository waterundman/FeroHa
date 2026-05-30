export interface TaskDispatchPayload {
  content: string;
  intent: string;
  blockId?: string;
}

export interface TaskDispatchResult {
  task_id: string;
  status: string;
}

export interface ResearchCompletedPayload {
  task_id: string;
  source_block_id?: string;
  intent: string;
  content: string;
  result: string;
}

/**
 * Send a task to the Rust backend agent via IPC.
 * Maps to the dispatch_agent_task Rust command.
 */
export async function sendTaskToAgent(
  payload: TaskDispatchPayload
): Promise<TaskDispatchResult | undefined> {
  try {
    const { invoke } = await import('@tauri-apps/api/core');
    const response = await invoke<TaskDispatchResult>('dispatch_agent_task', {
      payload: {
        content: payload.content,
        intent: payload.intent,
        blockId: payload.blockId ?? null,
        timestamp: Date.now(),
      },
    });
    return response;
  } catch (error) {
    console.error("Failed to dispatch to FeroHa Core:", error);
    return undefined;
  }
}

/**
 * Listen for research completion events from the Rust backend.
 * Returns an unlisten function to clean up the listener.
 */
export async function listenForResearchCompletion(
  callback: (payload: ResearchCompletedPayload) => void
): Promise<() => void> {
  const { listen } = await import('@tauri-apps/api/event');
  const unlisten = await listen<ResearchCompletedPayload>(
    'feroha_research_completed',
    (event) => {
      callback(event.payload);
    }
  );
  return unlisten;
}

/**
 * Rename a note file (move/rename within vault).
 */
export async function renameNote(oldPath: string, newPath: string): Promise<void> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke("rename_note", { oldPath, newPath });
}
