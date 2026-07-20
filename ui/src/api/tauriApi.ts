// Real backend client: invokes the src-tauri commands over Tauri IPC.
//
// Workflow execution uses one ordered Channel per invocation. The command
// result is the sole terminal authority; cancellation remains a request until
// that result reports cancelled, succeeded, or failed.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  AssetDto,
  AssetKind,
  AssetListPageDto,
  AssetPreviewDto,
  AssistantApprovalDecisionInput,
  AssistantPendingWorkflowChange,
  AssistantPresentationEvent,
  AssistantSendMessageResult,
  AssistantWorkflowChangeDecisionResult,
  AssistantSendInput,
  CapabilityRef,
  GenerationProfileForCapability,
  GenerationProviderSettingsActionDto,
  GenerationProviderSettingsDto,
  AssistantProviderModelsDto,
  AssistantProviderSettingsDto,
  GenerationTaskDto,
  GenerationTaskListPageDto,
  GenerationTaskRequestKindDto,
  GenerationTaskStatusDto,
  NodeCapabilityContractDto,
  Project,
  ProjectWorkspace,
  WorkflowApi,
  WorkflowDto,
  WorkflowMutationActionDto,
  WorkflowNodePresentationDto,
  WorkflowReadinessDto,
  WorkflowRunDto,
  WorkflowRunEventPageDto,
  WorkflowRunScopeDto,
  WorkflowWithReadinessDto,
} from "./types.ts";

async function assetImport(
  projectId: string,
  expectedMediaKind: AssetKind,
): Promise<AssetDto | null> {
  return invoke("asset_import", {
    request: {
      project_id: projectId,
      expected_media_kind: expectedMediaKind,
    },
  });
}

async function assetGet(projectId: string, assetId: string): Promise<AssetDto> {
  return invoke("asset_get", {
    request: { project_id: projectId, asset_id: assetId },
  });
}

async function assetList(
  projectId: string,
  mediaKind: AssetKind | null = null,
  cursor: string | null = null,
  limit = 100,
): Promise<AssetListPageDto> {
  return invoke("asset_list", {
    request: {
      project_id: projectId,
      media_kind: mediaKind,
      cursor,
      limit,
    },
  });
}

async function assetIssuePreview(
  projectId: string,
  assetId: string,
): Promise<AssetPreviewDto> {
  return invoke("asset_issue_preview", {
    request: { project_id: projectId, asset_id: assetId },
  });
}

async function listProjects(): Promise<Project[]> {
  const projects = new Map<string, Project>();
  let cursor: string | null = null;
  do {
    const page: { projects: Project[]; next_cursor: string | null } = await invoke(
      "project_list",
      { request: { limit: 100, cursor } },
    );
    for (const project of page.projects) projects.set(project.id, project);
    cursor = page.next_cursor;
  } while (cursor !== null);
  return [...projects.values()];
}

async function createProject(name: string): Promise<Project> {
  return invoke<Project>("project_create", {
    request: { request_id: crypto.randomUUID(), name },
  });
}

async function getProject(id: string): Promise<Project> {
  return invoke<Project>("project_get", { request: { project_id: id } });
}

async function renameProject(project: Project, name: string): Promise<Project> {
  return invoke<Project>("project_rename", {
    request: {
      request_id: crypto.randomUUID(),
      project_id: project.id,
      expected_revision: project.revision,
      name,
    },
  });
}

async function openProject(id: string): Promise<ProjectWorkspace> {
  return invoke<ProjectWorkspace>("project_open", {
    request: { project_id: id },
  });
}

async function nodeCapabilityList(): Promise<NodeCapabilityContractDto[]> {
  return invoke<NodeCapabilityContractDto[]>("node_capability_list", { request: {} });
}

async function generationProfileListForCapability(
  reference: CapabilityRef,
): Promise<GenerationProfileForCapability[]> {
  return invoke<GenerationProfileForCapability[]>("generation_profile_list_for_capability", {
    request: {
      capability_id: reference.id,
      capability_version: reference.version,
    },
  });
}

async function generationProviderSettingsGet(): Promise<GenerationProviderSettingsDto> {
  return invoke<GenerationProviderSettingsDto>("generation_provider_settings_get", {
    request: {},
  });
}

async function generationProviderSettingsApply(
  expectedSettingsRevision: string,
  action: GenerationProviderSettingsActionDto,
): Promise<GenerationProviderSettingsDto> {
  return invoke<GenerationProviderSettingsDto>("generation_provider_settings_apply", {
    request: { expected_settings_revision: expectedSettingsRevision, action },
  });
}

async function assistantProviderSettingsGet(): Promise<AssistantProviderSettingsDto> {
  return invoke<AssistantProviderSettingsDto>("assistant_provider_settings_get", {
    request: {},
  });
}

async function assistantProviderModelsList(
  baseUrl: string,
  apiKey: string | null,
): Promise<AssistantProviderModelsDto> {
  return invoke<AssistantProviderModelsDto>("assistant_provider_models_list", {
    request: { base_url: baseUrl, api_key: apiKey },
  });
}

async function assistantProviderSettingsTestAndApply(
  expectedSettingsRevision: string,
  baseUrl: string,
  apiKey: string | null,
  modelId: string,
): Promise<AssistantProviderSettingsDto> {
  return invoke<AssistantProviderSettingsDto>("assistant_provider_settings_test_and_apply", {
    request: {
      expected_settings_revision: expectedSettingsRevision,
      base_url: baseUrl,
      api_key: apiKey,
      model_id: modelId,
    },
  });
}

async function assistantProviderSettingsDisable(
  expectedSettingsRevision: string,
): Promise<AssistantProviderSettingsDto> {
  return invoke<AssistantProviderSettingsDto>("assistant_provider_settings_disable", {
    request: { expected_settings_revision: expectedSettingsRevision },
  });
}

async function generationTaskGet(projectId: string, taskId: string): Promise<GenerationTaskDto> {
  return invoke<GenerationTaskDto>("generation_task_get", {
    request: { project_id: projectId, generation_task_id: taskId },
  });
}

async function generationTaskList(
  projectId: string,
  status: GenerationTaskStatusDto | null = null,
  requestKind: GenerationTaskRequestKindDto | null = null,
  cursor: string | null = null,
  limit = 100,
): Promise<GenerationTaskListPageDto> {
  return invoke<GenerationTaskListPageDto>("generation_task_list", {
    request: {
      project_id: projectId,
      status,
      request_kind: requestKind,
      cursor,
      limit,
    },
  });
}

async function workflowCreate(projectId: string): Promise<WorkflowDto> {
  return invoke("workflow_create", {
    request: { request_id: crypto.randomUUID(), project_id: projectId },
  });
}

async function workflowGetCurrent(projectId: string): Promise<WorkflowWithReadinessDto> {
  return invoke("workflow_get_current", { request: { project_id: projectId } });
}

async function workflowApplyMutation(
  projectId: string,
  workflowId: string,
  baseRevision: string,
  actions: WorkflowMutationActionDto[],
): Promise<WorkflowWithReadinessDto> {
  return invoke("workflow_apply_mutation", {
    request: {
      project_id: projectId,
      request_id: crypto.randomUUID(),
      workflow_id: workflowId,
      base_revision: baseRevision,
      actions,
    },
  });
}

async function workflowCheckReadiness(
  projectId: string,
  workflowId: string,
): Promise<WorkflowReadinessDto> {
  return invoke("workflow_check_readiness", {
    request: { project_id: projectId, workflow_id: workflowId },
  });
}

async function workflowStartRun(
  projectId: string,
  workflowId: string,
  workflowRevision: string,
  scope: WorkflowRunScopeDto,
): Promise<WorkflowRunDto> {
  return invoke("workflow_start_run", {
    request: {
      request_id: crypto.randomUUID(),
      project_id: projectId,
      workflow_id: workflowId,
      workflow_revision: workflowRevision,
      scope,
    },
  });
}

async function workflowCancelRun(
  projectId: string,
  workflowRunId: string,
): Promise<WorkflowRunDto> {
  return invoke("workflow_cancel_run", {
    request: { project_id: projectId, workflow_run_id: workflowRunId },
  });
}

async function workflowGetRun(
  projectId: string,
  workflowRunId: string,
): Promise<WorkflowRunDto> {
  return invoke("workflow_get_run", {
    request: { project_id: projectId, workflow_run_id: workflowRunId },
  });
}

async function workflowListRunEvents(
  projectId: string,
  workflowRunId: string,
  afterSequence: string | null = null,
  limit = 500,
): Promise<WorkflowRunEventPageDto> {
  return invoke("workflow_list_run_events", {
    request: {
      project_id: projectId,
      workflow_run_id: workflowRunId,
      after_sequence: afterSequence,
      limit,
    },
  });
}

async function observeWorkflowRunEvents(
  onEvent: (event: import("./types.ts").DurableWorkflowRunEventDto) => void,
): Promise<() => void> {
  return listen("workflow-run-event-v1", ({ payload }) => onEvent(
    payload as import("./types.ts").DurableWorkflowRunEventDto,
  ));
}

async function workflowGetNodePresentation(
  projectId: string,
  workflowId: string,
  nodeId: string,
): Promise<WorkflowNodePresentationDto> {
  return invoke("workflow_get_node_presentation", {
    request: {
      project_id: projectId,
      workflow_id: workflowId,
      node_id: nodeId,
    },
  });
}

async function assistantSendMessage(
  input: AssistantSendInput,
): Promise<AssistantSendMessageResult> {
  return invoke<AssistantSendMessageResult>("assistant_send_message", { request: input });
}

async function assistantDecideWorkflowChange(
  input: AssistantApprovalDecisionInput,
): Promise<AssistantWorkflowChangeDecisionResult> {
  return invoke<AssistantWorkflowChangeDecisionResult>("assistant_decide_workflow_change", {
    request: input,
  });
}

async function assistantGetPendingWorkflowChange(
  projectId: string,
): Promise<AssistantPendingWorkflowChange | null> {
  return invoke<AssistantPendingWorkflowChange | null>("assistant_get_pending_workflow_change", {
    request: { project_id: projectId },
  });
}

async function observeAssistantPresentationEvents(
  onEvent: (event: AssistantPresentationEvent) => void,
): Promise<() => void> {
  return listen<AssistantPresentationEvent>("assistant-presentation-event-v1", ({ payload }) => {
    onEvent(payload);
  });
}

export const tauriApi: WorkflowApi = {
  assetImport,
  assetGet,
  assetList,
  assetIssuePreview,
  listProjects,
  createProject,
  getProject,
  renameProject,
  openProject,
  nodeCapabilityList,
  generationProfileListForCapability,
  generationProviderSettingsGet,
  generationProviderSettingsApply,
  assistantProviderSettingsGet,
  assistantProviderModelsList,
  assistantProviderSettingsTestAndApply,
  assistantProviderSettingsDisable,
  generationTaskGet,
  generationTaskList,
  workflowCreate,
  workflowGetCurrent,
  workflowApplyMutation,
  workflowCheckReadiness,
  workflowStartRun,
  workflowCancelRun,
  workflowGetRun,
  workflowListRunEvents,
  observeWorkflowRunEvents,
  workflowGetNodePresentation,
  assistantSendMessage,
  assistantGetPendingWorkflowChange,
  assistantDecideWorkflowChange,
  observeAssistantPresentationEvents,
};
