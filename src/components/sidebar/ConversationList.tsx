import { useState, useEffect, useCallback, useMemo } from "react";
import type { ReactNode } from "react";
import { motion } from "framer-motion";
import {
  Archive,
  ArchiveRestore,
  ArrowLeft,
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
import { useNavigate } from "react-router-dom";
import { useTranslation } from "react-i18next";
import {
  archiveConversation,
  deleteConversation,
  getConversations,
  getProjects,
  moveConversationToProject,
  pinConversation,
  renameConversation,
  toAssetUrl,
  unarchiveConversation,
  unpinConversation,
} from "../../lib/api";
import { formatConversationTime } from "../../lib/utils";
import type { Conversation, Project } from "../../types";
import ConfirmDialog from "../common/ConfirmDialog";

interface ConversationListProps {
  activeProjectId: string | null;
  activeConversationId: string | null;
  refreshKey: number;
  onSelectProject: (id: string | null) => void;
  onProjectCreated: (id: string) => void;
  onSelectConversation: (id: string) => void;
  onInitialConversation: (id: string) => void;
  onClearActiveConversation: () => void;
  onNewConversation: () => void;
}

type ActionMenu =
  | { type: "conversation"; id: string }
  | null;

export default function ConversationList({
  activeProjectId,
  activeConversationId,
  refreshKey,
  onSelectProject,
  onSelectConversation,
  onInitialConversation,
  onClearActiveConversation,
  onNewConversation,
}: ConversationListProps) {
  const navigate = useNavigate();
  const [projects, setProjects] = useState<Project[]>([]);
  const [conversations, setConversations] = useState<Conversation[]>([]);
  const [query, setQuery] = useState("");
  const [openMenu, setOpenMenu] = useState<ActionMenu>(null);
  const [deleteTarget, setDeleteTarget] = useState<Conversation | null>(null);
  const [deleteLoading, setDeleteLoading] = useState(false);
  const [deleteError, setDeleteError] = useState<string | null>(null);
  const { t } = useTranslation();

  const loadProjects = useCallback(async () => {
    const items = await getProjects(false);
    setProjects(items);
  }, []);

  const loadConversations = useCallback(
    async (q?: string) => {
      const items = await getConversations(
        q,
        activeProjectId || "default",
        false,
      );
      setConversations(items);
      if (!q && !activeConversationId && items.length > 0) {
        onInitialConversation(items[0].id);
      }
    },
    [activeConversationId, activeProjectId, onInitialConversation],
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
        setDeleteTarget(conversation);
        setDeleteError(null);
        return;
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

  const confirmDeleteConversation = useCallback(async () => {
    if (!deleteTarget) return;
    setDeleteLoading(true);
    setDeleteError(null);

    try {
      await deleteConversation(deleteTarget.id);
      if (activeConversationId === deleteTarget.id) {
        onClearActiveConversation();
      }
      setDeleteTarget(null);
      await loadAll(query);
    } catch {
      setDeleteError(t("projects.deleteConversationError"));
    } finally {
      setDeleteLoading(false);
    }
  }, [
    activeConversationId,
    deleteTarget,
    loadAll,
    onClearActiveConversation,
    query,
    t,
  ]);

  return (
    <div className="flex h-full flex-col bg-gradient-to-b from-surface/92 via-surface/74 to-surface-muted/52">
      <div className="px-4 pb-3 pt-5">
        <div className="mb-3 flex items-center justify-between gap-2">
          <div className="flex items-center gap-2">
            <MessageSquare size={13} className="text-muted" strokeWidth={1.8} />
            <span className="text-[13px] font-semibold text-foreground tracking-tight">
              {activeProjectId && selectedProject
                ? selectedProject.name
                : t("sidebar.conversations")}
            </span>
          </div>
          {activeProjectId && selectedProject ? (
            <button
              type="button"
              onClick={() => navigate("/projects")}
              className="focus-ring inline-flex items-center gap-1.5 rounded-full border border-border-subtle bg-surface px-2.5 py-1.5 text-[11px] font-medium text-foreground/80 shadow-card transition-colors hover:bg-subtle hover:text-foreground"
              aria-label={t("projects.backToList")}
            >
              <ArrowLeft size={12} strokeWidth={1.9} />
              <span>{t("projects.backToList")}</span>
            </button>
          ) : null}
        </div>
        <div className="relative">
          <Search size={12} className="absolute left-2.5 top-1/2 -translate-y-1/2 text-muted" strokeWidth={2} />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={t("sidebar.search")}
            className="h-8 w-full rounded-full border border-border-subtle bg-surface/72 pl-7 pr-2 text-[12px] text-foreground shadow-inset placeholder:text-muted/60 transition-colors focus:border-primary/24 focus:bg-surface focus:outline-none"
          />
        </div>
      </div>

      <div className="flex-1 overflow-y-auto px-2.5 pb-4 pt-3">
        <button
          onClick={onNewConversation}
          className="focus-ring mb-3 flex w-full items-center gap-2.5 rounded-[14px] border border-dashed border-border-subtle bg-surface/42 px-2 py-2 text-left transition-all hover:border-primary/30 hover:bg-surface hover:shadow-card"
        >
          <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-[10px] border border-border-subtle bg-surface shadow-card">
            <Plus size={14} className="text-primary" strokeWidth={2} />
          </div>
          <div className="min-w-0 flex-1">
            <p className="truncate text-[12px] leading-snug text-foreground">
              {t("sidebar.newConversation")}
            </p>
            <div className="mt-0.5 flex items-center gap-1.5">
              <span className="truncate text-[10px] text-muted/60">
                {activeProjectId ? t("sidebar.conversations") : selectedProject?.name ?? t("sidebar.conversations")}
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
          <div className="flex flex-col gap-1.5">
            {conversations.map((conv, i) => (
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
        )}
      </div>

      <ConfirmDialog
        open={deleteTarget !== null}
        title={t("sidebar.deleteConversationConfirm")}
        confirmLabel={t("projects.deleteConfirmAction")}
        cancelLabel={t("projects.deleteCancel")}
        loading={deleteLoading}
        error={deleteError}
        onConfirm={() => void confirmDeleteConversation()}
        onCancel={() => {
          if (!deleteLoading) {
            setDeleteTarget(null);
            setDeleteError(null);
          }
        }}
      />
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
      data-active-conversation={active ? "true" : "false"}
      className={`group relative rounded-[14px] border transition-all duration-200 ${
        active
          ? "border-white/72 bg-surface shadow-card ring-1 ring-primary/12"
          : "border-transparent hover:border-border-subtle hover:bg-surface/72 hover:shadow-card"
      }`}
    >
      <button
        onClick={onSelect}
        className="flex w-full items-center gap-2.5 rounded-[14px] px-2 py-2 text-left"
      >
        <div className="h-[43px] w-[43px] shrink-0 overflow-hidden rounded-[10px] border border-border-subtle bg-subtle shadow-[inset_0_0_0_1px_rgba(255,255,255,0.42)]">
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
            <p className={`truncate text-[12px] leading-snug transition-colors ${
              active ? "font-semibold text-foreground" : "text-foreground/78 group-hover:text-foreground"
            }`}>
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
              <span className="rounded-[5px] bg-primary/8 px-1.5 text-[9px] font-medium text-primary">
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
        className={`absolute right-1.5 top-1/2 flex h-7 w-7 -translate-y-1/2 items-center justify-center rounded-[9px] text-muted/60 transition-all hover:bg-subtle hover:text-foreground ${
          menuOpen ? "bg-subtle opacity-100" : "opacity-0 group-hover:opacity-100"
        }`}
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
