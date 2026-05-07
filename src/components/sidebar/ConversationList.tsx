import { useState, useEffect, useCallback, useMemo } from "react";
import type { ReactNode } from "react";
import { motion } from "framer-motion";
import {
  Archive,
  ArchiveRestore,
  FolderKanban,
  Image as ImageIcon,
  MessageSquare,
  MoreHorizontal,
  Pencil,
  Pin,
  PinOff,
  Plus,
  Search,
  Trash2,
} from "lucide-react";
import { useTranslation } from "react-i18next";
import {
  archiveConversation,
  archiveProject,
  createProject,
  deleteConversation,
  deleteProject,
  getConversations,
  getProjects,
  moveConversationToProject,
  pinConversation,
  pinProject,
  renameConversation,
  renameProject,
  toAssetUrl,
  unarchiveConversation,
  unarchiveProject,
  unpinConversation,
  unpinProject,
} from "../../lib/api";
import { formatConversationTime } from "../../lib/utils";
import type { Conversation, Project } from "../../types";

interface ConversationListProps {
  activeProjectId: string | null;
  activeConversationId: string | null;
  refreshKey: number;
  onSelectProject: (id: string | null) => void;
  onProjectCreated: (id: string) => void;
  onSelectConversation: (id: string) => void;
  onInitialConversation: (id: string) => void;
  onNewConversation: () => void;
}

type ActionMenu =
  | { type: "conversation"; id: string }
  | { type: "project"; id: string }
  | null;

function groupByProject(conversations: Conversation[], projects: Project[]) {
  const projectNames = new Map(projects.map((project) => [project.id, project.name]));
  const orderedIds = new Set(projects.map((project) => project.id));
  const groups = projects.map((project) => ({
    project,
    items: conversations.filter((conv) => conv.project_id === project.id),
  }));
  const remaining = conversations.filter((conv) => !orderedIds.has(conv.project_id));

  for (const conv of remaining) {
    const groupIndex = groups.findIndex((item) => item.project.id === conv.project_id);

    if (groupIndex === -1) {
      groups.push({
        project: {
          id: conv.project_id,
          name: conv.project_name ?? projectNames.get(conv.project_id) ?? "Project",
          created_at: conv.created_at,
          updated_at: conv.updated_at,
          archived_at: null,
          pinned_at: null,
          deleted_at: null,
          conversation_count: 0,
          image_count: 0,
        } satisfies Project,
        items: [],
      });
    }

    const targetGroup = groups[groupIndex === -1 ? groups.length - 1 : groupIndex];
    targetGroup.items.push(conv);
  }

  return groups.filter((group) => group.items.length > 0 || group.project.id === "default");
}

export default function ConversationList({
  activeProjectId,
  activeConversationId,
  refreshKey,
  onSelectProject,
  onProjectCreated,
  onSelectConversation,
  onInitialConversation,
  onNewConversation,
}: ConversationListProps) {
  const [projects, setProjects] = useState<Project[]>([]);
  const [conversations, setConversations] = useState<Conversation[]>([]);
  const [query, setQuery] = useState("");
  const [includeArchived, setIncludeArchived] = useState(false);
  const [openMenu, setOpenMenu] = useState<ActionMenu>(null);
  const { t } = useTranslation();

  const loadProjects = useCallback(async () => {
    const items = await getProjects(includeArchived);
    setProjects(items);
  }, [includeArchived]);

  const loadConversations = useCallback(
    async (q?: string) => {
      const items = await getConversations(
        q,
        activeProjectId,
        includeArchived,
      );
      setConversations(items);
      if (!q && !activeConversationId && items.length > 0) {
        onInitialConversation(items[0].id);
      }
    },
    [activeConversationId, activeProjectId, includeArchived, onInitialConversation],
  );

  const loadAll = useCallback(
    async (q?: string) => {
      await Promise.all([loadProjects(), loadConversations(q)]);
    },
    [loadConversations, loadProjects],
  );

  useEffect(() => {
    if (query) {
      const timer = setTimeout(() => {
        loadAll(query).catch(() => {});
      }, 300);
      return () => clearTimeout(timer);
    }

    loadAll().catch(() => {});
  }, [query, refreshKey, loadAll]);

  const selectedProject = useMemo(
    () => projects.find((project) => project.id === activeProjectId) ?? null,
    [activeProjectId, projects],
  );
  const visibleGroups = groupByProject(conversations, projects);

  const runConversationAction = useCallback(
    async (conversation: Conversation, action: string) => {
      setOpenMenu(null);
      if (action === "rename") {
        const title = window.prompt(t("sidebar.renameConversation"), conversation.title);
        if (!title || title.trim() === conversation.title) return;
        await renameConversation(conversation.id, title.trim());
      } else if (action === "pin") {
        await pinConversation(conversation.id);
      } else if (action === "unpin") {
        await unpinConversation(conversation.id);
      } else if (action === "archive") {
        await archiveConversation(conversation.id);
        if (activeConversationId === conversation.id) {
          onSelectProject(activeProjectId);
        }
      } else if (action === "unarchive") {
        await unarchiveConversation(conversation.id);
      } else if (action === "delete") {
        if (!window.confirm(t("sidebar.deleteConversationConfirm"))) return;
        await deleteConversation(conversation.id);
        if (activeConversationId === conversation.id) {
          onSelectProject(activeProjectId);
        }
      } else if (action.startsWith("move:")) {
        const projectId = action.slice("move:".length);
        await moveConversationToProject(conversation.id, projectId);
        onSelectProject(projectId);
      }
      await loadAll(query);
    },
    [
      activeConversationId,
      activeProjectId,
      loadAll,
      onSelectConversation,
      onSelectProject,
      query,
      t,
    ],
  );

  const runProjectAction = useCallback(
    async (project: Project, action: string) => {
      setOpenMenu(null);
      if (action === "rename") {
        const name = window.prompt(t("sidebar.renameProject"), project.name);
        if (!name || name.trim() === project.name) return;
        await renameProject(project.id, name.trim());
      } else if (action === "pin") {
        await pinProject(project.id);
      } else if (action === "unpin") {
        await unpinProject(project.id);
      } else if (action === "archive") {
        await archiveProject(project.id);
        if (activeProjectId === project.id) {
          onSelectProject(null);
        }
      } else if (action === "unarchive") {
        await unarchiveProject(project.id);
      } else if (action === "delete") {
        if (!window.confirm(t("sidebar.deleteProjectConfirm"))) return;
        await deleteProject(project.id);
        if (activeProjectId === project.id) {
          onSelectProject(null);
        }
      }
      await loadAll(query);
    },
    [activeProjectId, loadAll, onSelectProject, query, t],
  );

  const handleCreateProject = useCallback(async () => {
    const name = window.prompt(t("sidebar.newProject"));
    if (!name?.trim()) return;
    const project = await createProject(name.trim());
    onProjectCreated(project.id);
    await loadAll(query);
  }, [loadAll, onProjectCreated, query, t]);

  return (
    <div className="flex h-full flex-col">
      <div className="px-4 pt-5 pb-3">
        <div className="mb-3 flex items-center justify-between gap-2">
          <div className="flex items-center gap-2">
            <MessageSquare size={13} className="text-muted" strokeWidth={1.8} />
            <span className="text-[13px] font-semibold text-foreground tracking-tight">
              {t("sidebar.conversations")}
            </span>
          </div>
          <button
            onClick={handleCreateProject}
            className="flex h-7 w-7 items-center justify-center rounded-[8px] text-muted transition-colors hover:bg-subtle hover:text-foreground"
            title={t("sidebar.newProject")}
            aria-label={t("sidebar.newProject")}
          >
            <FolderKanban size={14} />
          </button>
        </div>
        <div className="relative">
          <Search size={12} className="absolute left-2.5 top-1/2 -translate-y-1/2 text-muted" strokeWidth={2} />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={t("sidebar.search")}
            className="h-[28px] w-full rounded-[8px] border border-border-subtle bg-subtle/50 pl-7 pr-2 text-[12px] text-foreground placeholder:text-muted/60 focus:outline-none focus:border-border focus:bg-surface transition-colors"
          />
        </div>
      </div>

      <div className="border-y border-border-subtle bg-subtle/25 px-3 py-2">
        <div className="mb-2 flex items-center justify-between gap-2">
          <span className="text-[10px] font-semibold uppercase tracking-wider text-muted/60">
            {t("sidebar.projects")}
          </span>
          <button
            onClick={() => setIncludeArchived((value) => !value)}
            className={`text-[10px] font-medium transition-colors ${
              includeArchived ? "text-primary" : "text-muted/70 hover:text-foreground"
            }`}
          >
            {t("sidebar.archived")}
          </button>
        </div>
        <div className="flex gap-1.5 overflow-x-auto pb-0.5">
          <button
            onClick={() => onSelectProject(null)}
            className={`max-w-[150px] shrink-0 rounded-[8px] border px-2 py-1 text-left text-[11px] font-medium transition-colors ${
              activeProjectId === null
                ? "border-primary/20 bg-primary/8 text-primary"
                : "border-border-subtle bg-surface text-foreground/70 hover:text-foreground"
            }`}
            title={t("sidebar.allProjects")}
          >
            <span className="block truncate">{t("sidebar.allProjects")}</span>
          </button>
          {projects.map((project) => (
            <button
              key={project.id}
              onClick={() => onSelectProject(project.id)}
              className={`max-w-[150px] shrink-0 rounded-[8px] border px-2 py-1 text-left text-[11px] font-medium transition-colors ${
                activeProjectId === project.id
                  ? "border-primary/20 bg-primary/8 text-primary"
                  : "border-border-subtle bg-surface text-foreground/70 hover:text-foreground"
              }`}
              title={project.name}
            >
              <span className="block truncate">{project.name}</span>
            </button>
          ))}
        </div>
      </div>

      <div className="flex-1 overflow-y-auto px-2.5 pb-4 pt-3">
        <button
          onClick={onNewConversation}
          className="mb-3 flex w-full items-center gap-2.5 rounded-[10px] border border-dashed border-border-subtle px-2 py-2 text-left transition-all hover:border-primary/30 hover:bg-primary/4"
        >
          <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-[8px] border border-border-subtle bg-subtle">
            <Plus size={14} className="text-primary" strokeWidth={2} />
          </div>
          <div className="min-w-0 flex-1">
            <p className="truncate text-[12px] leading-snug text-foreground">
              {t("sidebar.newConversation")}
            </p>
            <div className="mt-0.5 flex items-center gap-1.5">
              <span className="truncate text-[10px] text-muted/60">
                {selectedProject?.name ?? t("sidebar.projects")}
              </span>
            </div>
          </div>
        </button>

        {conversations.length === 0 ? (
          <div className="px-2 pt-6 text-center">
            <p className="text-[12px] text-muted/50">
              {query ? t("sidebar.noResults") : t("sidebar.noConversations")}
            </p>
          </div>
        ) : (
          visibleGroups.map((group) => (
            <div key={group.project.id} className="mb-4">
              <div className="mb-1 flex items-center justify-between gap-2 px-2">
                <button
                  onClick={() => onSelectProject(group.project.id)}
                  className="min-w-0 text-left"
                >
                  <p className="truncate text-[10px] font-medium uppercase tracking-wider text-muted/50">
                    {group.project.name}
                  </p>
                </button>
                <div className="relative">
                  <button
                    onClick={() =>
                      setOpenMenu((current) =>
                        current?.type === "project" && current.id === group.project.id
                          ? null
                          : { type: "project", id: group.project.id },
                      )
                    }
                    className="flex h-6 w-6 items-center justify-center rounded-[7px] text-muted/60 transition-colors hover:bg-subtle hover:text-foreground"
                    aria-label={t("sidebar.projectActions")}
                  >
                    <MoreHorizontal size={14} />
                  </button>
                  {openMenu?.type === "project" && openMenu.id === group.project.id && (
                    <ActionPopover>
                      <ActionButton
                        icon={<Pencil size={13} />}
                        label={t("sidebar.rename")}
                        onClick={() => void runProjectAction(group.project, "rename")}
                      />
                      <ActionButton
                        icon={group.project.pinned_at ? <PinOff size={13} /> : <Pin size={13} />}
                        label={
                          group.project.pinned_at
                            ? t("sidebar.unpin")
                            : t("sidebar.pin")
                        }
                        onClick={() =>
                          void runProjectAction(
                            group.project,
                            group.project.pinned_at ? "unpin" : "pin",
                          )
                        }
                      />
                      <ActionButton
                        icon={
                          group.project.archived_at
                            ? <ArchiveRestore size={13} />
                            : <Archive size={13} />
                        }
                        label={
                          group.project.archived_at
                            ? t("sidebar.unarchive")
                            : t("sidebar.archive")
                        }
                        onClick={() =>
                          void runProjectAction(
                            group.project,
                            group.project.archived_at ? "unarchive" : "archive",
                          )
                        }
                      />
                      {group.project.id !== "default" && (
                        <ActionButton
                          danger
                          icon={<Trash2 size={13} />}
                          label={t("sidebar.delete")}
                          onClick={() => void runProjectAction(group.project, "delete")}
                        />
                      )}
                    </ActionPopover>
                  )}
                </div>
              </div>

              <div className="flex flex-col gap-0.5">
                {group.items.map((conv, i) => (
                  <ConversationRow
                    key={conv.id}
                    conversation={conv}
                    active={activeConversationId === conv.id}
                    index={i}
                    menuOpen={
                      openMenu?.type === "conversation" && openMenu.id === conv.id
                    }
                    onSelect={() => onSelectConversation(conv.id)}
                    onToggleMenu={() =>
                      setOpenMenu((current) =>
                        current?.type === "conversation" && current.id === conv.id
                          ? null
                          : { type: "conversation", id: conv.id },
                      )
                    }
                    onAction={(action) => void runConversationAction(conv, action)}
                    projects={projects}
                  />
                ))}
              </div>
            </div>
          ))
        )}
      </div>
    </div>
  );
}

function ConversationRow({
  conversation,
  active,
  index,
  menuOpen,
  onSelect,
  onToggleMenu,
  onAction,
  projects,
}: {
  conversation: Conversation;
  active: boolean;
  index: number;
  menuOpen: boolean;
  onSelect: () => void;
  onToggleMenu: () => void;
  onAction: (action: string) => void;
  projects: Project[];
}) {
  const { t } = useTranslation();

  return (
    <motion.div
      initial={{ opacity: 0, x: -6 }}
      animate={{ opacity: 1, x: 0 }}
      transition={{ delay: index * 0.03, duration: 0.25 }}
      className={`group relative rounded-[10px] transition-colors hover:bg-subtle ${
        active ? "bg-primary/6" : ""
      }`}
    >
      <button
        onClick={onSelect}
        className="flex w-full items-center gap-2.5 rounded-[10px] px-2 py-2 text-left"
      >
        <div className="h-9 w-9 shrink-0 overflow-hidden rounded-[8px] bg-subtle border border-border-subtle">
          {conversation.latest_thumbnail ? (
            <img
              src={toAssetUrl(conversation.latest_thumbnail)}
              alt=""
              className="h-full w-full object-cover"
              loading="lazy"
            />
          ) : (
            <div className="flex h-full w-full items-center justify-center">
              <ImageIcon size={14} className="text-muted/30" />
            </div>
          )}
        </div>
        <div className="min-w-0 flex-1 pr-6">
          <div className="flex min-w-0 items-center gap-1.5">
            {conversation.pinned_at && <Pin size={10} className="shrink-0 text-primary" />}
            <p className="truncate text-[12px] leading-snug text-foreground/80 group-hover:text-foreground transition-colors">
              {conversation.title}
            </p>
          </div>
          <div className="mt-0.5 flex items-center gap-1.5">
            <span className="text-[10px] text-muted/60">
              {formatConversationTime(
                conversation.latest_generation_at ?? conversation.updated_at,
              )}
            </span>
            {conversation.generation_count > 1 && (
              <span className="rounded-[4px] bg-primary/8 px-1 text-[9px] font-medium text-primary">
                {conversation.generation_count}
              </span>
            )}
            {conversation.archived_at && (
              <span className="rounded-[4px] bg-muted/10 px-1 text-[9px] font-medium text-muted">
                {t("sidebar.archived")}
              </span>
            )}
          </div>
        </div>
      </button>
      <button
        onClick={(event) => {
          event.stopPropagation();
          onToggleMenu();
        }}
        className="absolute right-1.5 top-1/2 flex h-7 w-7 -translate-y-1/2 items-center justify-center rounded-[8px] text-muted/60 opacity-0 transition-all hover:bg-surface hover:text-foreground group-hover:opacity-100"
        aria-label={t("sidebar.conversationActions")}
      >
        <MoreHorizontal size={14} />
      </button>
      {menuOpen && (
        <ActionPopover>
          <ActionButton
            icon={<Pencil size={13} />}
            label={t("sidebar.rename")}
            onClick={() => onAction("rename")}
          />
          <ActionButton
            icon={conversation.pinned_at ? <PinOff size={13} /> : <Pin size={13} />}
            label={conversation.pinned_at ? t("sidebar.unpin") : t("sidebar.pin")}
            onClick={() => onAction(conversation.pinned_at ? "unpin" : "pin")}
          />
          <ActionButton
            icon={
              conversation.archived_at
                ? <ArchiveRestore size={13} />
                : <Archive size={13} />
            }
            label={
              conversation.archived_at ? t("sidebar.unarchive") : t("sidebar.archive")
            }
            onClick={() =>
              onAction(conversation.archived_at ? "unarchive" : "archive")
            }
          />
          <ActionButton
            danger
            icon={<Trash2 size={13} />}
            label={t("sidebar.delete")}
            onClick={() => onAction("delete")}
          />
          {projects.filter((project) => project.id !== conversation.project_id).length > 0 && (
            <>
              <div className="my-1 border-t border-border-subtle" />
              <p className="px-3 py-1 text-[10px] font-medium uppercase tracking-wider text-muted/50">
                {t("sidebar.moveToProject")}
              </p>
              {projects
                .filter((project) => project.id !== conversation.project_id)
                .slice(0, 6)
                .map((project) => (
                  <ActionButton
                    key={project.id}
                    icon={<FolderKanban size={13} />}
                    label={project.name}
                    onClick={() => onAction(`move:${project.id}`)}
                  />
                ))}
            </>
          )}
        </ActionPopover>
      )}
    </motion.div>
  );
}

function ActionPopover({ children }: { children: ReactNode }) {
  return (
    <div className="absolute right-0 top-7 z-20 w-36 overflow-hidden rounded-[10px] border border-border-subtle bg-surface py-1 shadow-[0_14px_35px_rgba(0,0,0,0.15)]">
      {children}
    </div>
  );
}

function ActionButton({
  icon,
  label,
  onClick,
  danger = false,
}: {
  icon: ReactNode;
  label: string;
  onClick: () => void;
  danger?: boolean;
}) {
  return (
    <button
      onClick={onClick}
      className={`flex w-full items-center gap-2 px-3 py-2 text-left text-[12px] transition-colors ${
        danger
          ? "text-error hover:bg-error/8"
          : "text-foreground/75 hover:bg-subtle hover:text-foreground"
      }`}
    >
      {icon}
      <span className="truncate">{label}</span>
    </button>
  );
}
