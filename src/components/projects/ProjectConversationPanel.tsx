import { useState } from "react";
import { motion } from "framer-motion";
import {
  Image as ImageIcon,
  MessageSquarePlus,
  MoreHorizontal,
  Pencil,
  Pin,
  PinOff,
  Trash2,
} from "lucide-react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import {
  createConversation,
  deleteConversation,
  pinConversation,
  renameConversation,
  toAssetUrl,
  unpinConversation,
} from "../../lib/api";
import { formatConversationTime } from "../../lib/utils";
import type { Conversation } from "../../types";
import ProjectNameDialog from "./ProjectNameDialog";
import ConfirmDialog from "../common/ConfirmDialog";

interface ProjectConversationPanelProps {
  projectId: string;
  conversations: Conversation[];
  onConversationsChange: (conversations: Conversation[]) => void;
}

export default function ProjectConversationPanel({
  projectId,
  conversations,
  onConversationsChange,
}: ProjectConversationPanelProps) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [openMenuId, setOpenMenuId] = useState<string | null>(null);
  const [renameTarget, setRenameTarget] = useState<Conversation | null>(null);
  const [renameLoading, setRenameLoading] = useState(false);
  const [renameError, setRenameError] = useState<string | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<Conversation | null>(null);
  const [deleteLoading, setDeleteLoading] = useState(false);
  const [deleteError, setDeleteError] = useState<string | null>(null);

  async function handleCreate() {
    try {
      const conversation = await createConversation(undefined, projectId);
      onConversationsChange([conversation, ...conversations]);
      navigate(`/projects/${projectId}/chat/${conversation.id}`);
    } catch {
      navigate(`/projects/${projectId}/chat`);
    }
  }

  async function handleRename(name: string) {
    if (!renameTarget) return;
    if (name === renameTarget.title) {
      setRenameTarget(null);
      setRenameError(null);
      return;
    }
    setRenameLoading(true);
    setRenameError(null);
    try {
      await renameConversation(renameTarget.id, name);
      setRenameTarget(null);
      onConversationsChange(
        conversations.map((c) =>
          c.id === renameTarget.id ? { ...c, title: name } : c,
        ),
      );
    } catch {
      setRenameError(t("projects.renameConversationError"));
    } finally {
      setRenameLoading(false);
    }
  }

  async function handlePin(conversation: Conversation) {
    setOpenMenuId(null);
    try {
      if (conversation.pinned_at) {
        await unpinConversation(conversation.id);
      } else {
        await pinConversation(conversation.id);
      }
      onConversationsChange(
        conversations.map((c) =>
          c.id === conversation.id
            ? { ...c, pinned_at: conversation.pinned_at ? null : new Date().toISOString() }
            : c,
        ),
      );
    } catch {
      // silently fail for pin toggle
    }
  }

  async function handleDelete() {
    if (!deleteTarget) return;
    setDeleteLoading(true);
    setDeleteError(null);
    try {
      await deleteConversation(deleteTarget.id);
      setDeleteTarget(null);
      onConversationsChange(conversations.filter((c) => c.id !== deleteTarget.id));
    } catch {
      setDeleteError(t("projects.deleteConversationError"));
    } finally {
      setDeleteLoading(false);
    }
  }

  const sortedConversations = [...conversations].sort((a, b) => {
    if (a.pinned_at && !b.pinned_at) return -1;
    if (!a.pinned_at && b.pinned_at) return 1;
    return new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime();
  });

  return (
    <div className="flex h-full flex-col">
      <div className="shrink-0 px-5 pt-6 pb-4">
        <div className="flex items-center justify-between gap-3">
          <div>
            <h2 className="text-[13px] font-semibold text-foreground tracking-tight">
              {t("projects.conversations")}
            </h2>
            <p className="mt-0.5 text-[11px] text-muted">
              {t("projects.conversationCount")} · {conversations.length}
            </p>
          </div>
          <button
            onClick={handleCreate}
            className="flex h-9 w-9 items-center justify-center rounded-[10px] bg-primary/8 text-primary transition-all hover:bg-primary/14 hover:scale-105 active:scale-95"
            aria-label={t("projects.newConversation")}
            title={t("projects.newConversation")}
          >
            <MessageSquarePlus size={16} strokeWidth={1.8} />
          </button>
        </div>
      </div>

      <div className="flex-1 overflow-y-auto px-3 pb-4">
        {sortedConversations.length === 0 ? (
          <div className="px-4 pt-16 text-center">
            <div className="mx-auto mb-3 flex h-12 w-12 items-center justify-center rounded-[12px] bg-subtle">
              <MessageSquarePlus size={20} className="text-muted/40" strokeWidth={1.6} />
            </div>
            <p className="text-[12px] text-muted/50 leading-relaxed">
              {t("projects.emptyConversations")}
            </p>
          </div>
        ) : (
          <div className="flex flex-col gap-0.5">
            {sortedConversations.map((conv, i) => (
              <motion.div
                key={conv.id}
                initial={{ opacity: 0, y: 4 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: i * 0.02, duration: 0.2 }}
                className="group relative"
              >
                <button
                  onClick={() => navigate(`/projects/${projectId}/chat/${conv.id}`)}
                  className="flex w-full items-center gap-3 rounded-[10px] px-2 py-2.5 text-left transition-colors hover:bg-subtle"
                >
                  <div className="flex h-10 w-10 shrink-0 items-center justify-center overflow-hidden rounded-[8px] bg-subtle border border-border-subtle">
                    {conv.latest_thumbnail ? (
                      <img
                        src={toAssetUrl(conv.latest_thumbnail)}
                        alt=""
                        className="h-full w-full object-cover"
                        loading="lazy"
                      />
                    ) : (
                      <ImageIcon size={15} className="text-muted/25" />
                    )}
                  </div>
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-1.5">
                      {conv.pinned_at && (
                        <Pin size={9} className="shrink-0 text-primary" />
                      )}
                      <p className="truncate text-[12.5px] font-medium text-foreground/85 group-hover:text-foreground transition-colors">
                        {conv.title}
                      </p>
                    </div>
                    <div className="mt-0.5 flex items-center gap-1.5">
                      <span className="text-[10.5px] text-muted/55">
                        {formatConversationTime(
                          conv.latest_generation_at ?? conv.updated_at,
                        )}
                      </span>
                      {conv.generation_count > 0 && (
                        <span className="rounded-[4px] bg-primary/6 px-1.5 py-px text-[9.5px] font-medium text-primary/70">
                          {conv.generation_count}
                        </span>
                      )}
                    </div>
                  </div>
                </button>

                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    setOpenMenuId(openMenuId === conv.id ? null : conv.id);
                  }}
                  className="absolute right-1.5 top-1/2 flex h-7 w-7 -translate-y-1/2 items-center justify-center rounded-[7px] text-muted/50 opacity-0 transition-all hover:bg-surface hover:text-foreground group-hover:opacity-100"
                  aria-label={t("sidebar.conversationActions")}
                >
                  <MoreHorizontal size={14} />
                </button>

                {openMenuId === conv.id && (
                  <div className="absolute right-1 top-9 z-20 w-36 overflow-hidden rounded-[10px] border border-border-subtle bg-surface py-1 shadow-[0_14px_35px_rgba(0,0,0,0.15)]">
                    <button
                      onClick={() => {
                        setOpenMenuId(null);
                        setRenameTarget(conv);
                      }}
                      className="flex w-full items-center gap-2 px-3 py-2 text-left text-[12px] text-foreground/75 hover:bg-subtle hover:text-foreground transition-colors"
                    >
                      <Pencil size={13} />
                      <span>{t("sidebar.rename")}</span>
                    </button>
                    <button
                      onClick={() => handlePin(conv)}
                      className="flex w-full items-center gap-2 px-3 py-2 text-left text-[12px] text-foreground/75 hover:bg-subtle hover:text-foreground transition-colors"
                    >
                      {conv.pinned_at ? <PinOff size={13} /> : <Pin size={13} />}
                      <span>{conv.pinned_at ? t("sidebar.unpin") : t("sidebar.pin")}</span>
                    </button>
                    <button
                      onClick={() => {
                        setOpenMenuId(null);
                        setDeleteTarget(conv);
                      }}
                      className="flex w-full items-center gap-2 px-3 py-2 text-left text-[12px] text-error hover:bg-error/8 transition-colors"
                    >
                      <Trash2 size={13} />
                      <span>{t("sidebar.delete")}</span>
                    </button>
                  </div>
                )}
              </motion.div>
            ))}
          </div>
        )}
      </div>

      <ProjectNameDialog
        open={renameTarget !== null}
        title={t("sidebar.renameConversation")}
        label={t("projectDialog.nameLabel")}
        initialName={renameTarget?.title ?? ""}
        submitLabel={t("projectDialog.renameSubmit")}
        cancelLabel={t("projectDialog.cancel")}
        requiredMessage={t("projectDialog.nameRequired")}
        error={renameError}
        loading={renameLoading}
        onSubmit={(name) => void handleRename(name)}
        onCancel={() => {
          if (!renameLoading) {
            setRenameTarget(null);
            setRenameError(null);
          }
        }}
      />

      <ConfirmDialog
        open={deleteTarget !== null}
        title={t("sidebar.deleteConversationConfirm")}
        confirmLabel={t("projects.deleteConfirmAction")}
        cancelLabel={t("projects.deleteCancel")}
        loading={deleteLoading}
        error={deleteError}
        onConfirm={() => void handleDelete()}
        onCancel={() => {
          if (!deleteLoading) {
            setDeleteTarget(null);
          }
        }}
      />
    </div>
  );
}
