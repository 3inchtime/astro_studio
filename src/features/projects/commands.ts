import { invoke } from "@tauri-apps/api/core";
import type { Conversation, Project, GenerationResult } from "../../types";

export async function createConversation(
  title?: string,
  projectId?: string | null,
): Promise<Conversation> {
  return invoke("create_conversation", {
    title: title || null,
    projectId: projectId || null,
  });
}

export async function getConversations(
  query?: string,
  projectId?: string | null,
  includeArchived?: boolean,
): Promise<Conversation[]> {
  return invoke("get_conversations", {
    query: query || null,
    projectId: projectId || null,
    includeArchived: includeArchived || null,
  });
}

export async function getConversationGenerations(
  conversationId: string,
): Promise<GenerationResult[]> {
  return invoke("get_conversation_generations", { conversationId });
}

export async function renameConversation(
  id: string,
  title: string,
): Promise<void> {
  await invoke("rename_conversation", { id, title });
}

export async function moveConversationToProject(
  id: string,
  projectId: string,
): Promise<void> {
  await invoke("move_conversation_to_project", { id, projectId });
}

export async function archiveConversation(id: string): Promise<void> {
  await invoke("archive_conversation", { id });
}

export async function unarchiveConversation(id: string): Promise<void> {
  await invoke("unarchive_conversation", { id });
}

export async function pinConversation(id: string): Promise<void> {
  await invoke("pin_conversation", { id });
}

export async function unpinConversation(id: string): Promise<void> {
  await invoke("unpin_conversation", { id });
}

export async function deleteConversation(id: string): Promise<void> {
  await invoke("delete_conversation", { id });
}

export async function createProject(name?: string): Promise<Project> {
  return invoke("create_project", { name: name || null });
}

export async function getProjects(
  includeArchived?: boolean,
): Promise<Project[]> {
  return invoke("get_projects", { includeArchived: includeArchived || null });
}

export async function renameProject(id: string, name: string): Promise<void> {
  await invoke("rename_project", { id, name });
}

export async function archiveProject(id: string): Promise<void> {
  await invoke("archive_project", { id });
}

export async function unarchiveProject(id: string): Promise<void> {
  await invoke("unarchive_project", { id });
}

export async function pinProject(id: string): Promise<void> {
  await invoke("pin_project", { id });
}

export async function unpinProject(id: string): Promise<void> {
  await invoke("unpin_project", { id });
}

export async function deleteProject(id: string): Promise<void> {
  await invoke("delete_project", { id });
}
