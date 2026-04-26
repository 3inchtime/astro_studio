import { useState, useEffect, useCallback } from "react";
import { motion } from "framer-motion";
import { Search, Image as ImageIcon, MessageSquare, Plus } from "lucide-react";
import { useTranslation } from "react-i18next";
import { getConversations, toAssetUrl } from "../../lib/api";
import { formatConversationTime, toLocalDate } from "../../lib/utils";
import type { Conversation } from "../../types";

interface ConversationListProps {
  activeConversationId: string | null;
  refreshKey: number;
  onSelectConversation: (id: string) => void;
  onInitialConversation: (id: string) => void;
  onNewConversation: () => void;
}

function groupByDate(conversations: Conversation[], t: (key: string) => string) {
  const now = new Date();
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const yesterday = new Date(today.getTime() - 86400000);
  const weekAgo = new Date(today.getTime() - 7 * 86400000);

  const groups: { label: string; items: Conversation[] }[] = [
    { label: t("sidebar.today"), items: [] },
    { label: t("sidebar.yesterday"), items: [] },
    { label: t("sidebar.previous7Days"), items: [] },
    { label: t("sidebar.older"), items: [] },
  ];

  for (const conv of conversations) {
    const date = toLocalDate(conv.latest_generation_at ?? conv.updated_at);
    if (date >= today) groups[0].items.push(conv);
    else if (date >= yesterday) groups[1].items.push(conv);
    else if (date >= weekAgo) groups[2].items.push(conv);
    else groups[3].items.push(conv);
  }

  return groups.filter((g) => g.items.length > 0);
}

export default function ConversationList({
  activeConversationId,
  refreshKey,
  onSelectConversation,
  onInitialConversation,
  onNewConversation,
}: ConversationListProps) {
  const [conversations, setConversations] = useState<Conversation[]>([]);
  const [query, setQuery] = useState("");
  const { t } = useTranslation();

  const load = useCallback((q?: string) => {
    getConversations(q).then((items) => {
      setConversations(items);
      if (!q && !activeConversationId && items.length > 0) {
        onInitialConversation(items[0].id);
      }
    }).catch(() => {});
  }, [activeConversationId, onInitialConversation]);

  useEffect(() => {
    if (query) {
      const timer = setTimeout(() => load(query), 300);
      return () => clearTimeout(timer);
    } else {
      load();
    }
  }, [query, refreshKey, load]);

  const groups = groupByDate(conversations, t);

  return (
    <div className="flex h-full flex-col">
      <div className="px-4 pt-5 pb-3">
        <div className="mb-3 flex items-center gap-2">
          <MessageSquare size={13} className="text-muted" strokeWidth={1.8} />
          <span className="text-[13px] font-semibold text-foreground tracking-tight">
            {t("sidebar.conversations")}
          </span>
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

      <div className="flex-1 overflow-y-auto px-2.5 pb-4">
        <button
          onClick={onNewConversation}
          className="mb-3 flex w-full items-center gap-2.5 rounded-[10px] border border-dashed border-border-subtle px-2 py-2 text-left transition-all hover:border-primary/30 hover:bg-primary/4"
        >
          <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-[8px] border border-border-subtle bg-subtle">
            <Plus size={14} className="text-primary" strokeWidth={2} />
          </div>
          <div className="min-w-0 flex-1">
            <p className="text-[12px] leading-snug text-foreground">
              {t("sidebar.newConversation")}
            </p>
            <div className="mt-0.5 flex items-center gap-1.5">
              <span className="text-[10px] text-muted/60">
                {t("sidebar.conversations")}
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
          groups.map((group) => (
            <div key={group.label} className="mb-3">
              <p className="px-2 pb-1 text-[10px] font-medium uppercase tracking-wider text-muted/50">
                {group.label}
              </p>
              <div className="flex flex-col gap-0.5">
                {group.items.map((conv, i) => (
                  <motion.button
                    key={conv.id}
                    initial={{ opacity: 0, x: -6 }}
                    animate={{ opacity: 1, x: 0 }}
                    transition={{ delay: i * 0.03, duration: 0.25 }}
                    onClick={() => onSelectConversation(conv.id)}
                    className={`group flex items-center gap-2.5 rounded-[10px] px-2 py-2 text-left transition-colors hover:bg-subtle ${
                      activeConversationId === conv.id ? "bg-primary/6" : ""
                    }`}
                  >
                    <div className="h-9 w-9 shrink-0 overflow-hidden rounded-[8px] bg-subtle border border-border-subtle">
                      {conv.latest_thumbnail ? (
                        <img
                          src={toAssetUrl(conv.latest_thumbnail)}
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
                    <div className="min-w-0 flex-1">
                      <p className="truncate text-[12px] leading-snug text-foreground/80 group-hover:text-foreground transition-colors">
                        {conv.title}
                      </p>
                      <div className="mt-0.5 flex items-center gap-1.5">
                        <span className="text-[10px] text-muted/60">
                          {formatConversationTime(conv.latest_generation_at ?? conv.updated_at)}
                        </span>
                        {conv.generation_count > 1 && (
                          <span className="rounded-[4px] bg-primary/8 px-1 text-[9px] font-medium text-primary">
                            {conv.generation_count}
                          </span>
                        )}
                      </div>
                    </div>
                  </motion.button>
                ))}
              </div>
            </div>
          ))
        )}
      </div>
    </div>
  );
}
