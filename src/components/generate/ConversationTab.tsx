import { X } from "lucide-react";

interface Tab {
  id: string;
  title: string;
}

interface ConversationTabProps {
  tabs: Tab[];
  activeId: string | null;
  onSelect: (id: string) => void;
  onClose: (id: string) => void;
}

export default function ConversationTab({ tabs, activeId, onSelect, onClose }: ConversationTabProps) {
  if (tabs.length <= 1) return null;

  return (
    <div className="flex items-center gap-1 border-b border-border-subtle bg-surface/80 px-4 py-2 overflow-x-auto backdrop-blur-sm">
      {tabs.map((tab) => (
        <div
          key={tab.id}
          onClick={() => onSelect(tab.id)}
          className={`flex shrink-0 items-center gap-1.5 rounded-[8px] px-2.5 py-1 text-[11px] font-medium cursor-pointer transition-all duration-200 ${
            tab.id === activeId
              ? "bg-primary/6 text-primary shadow-card"
              : "text-muted hover:bg-subtle hover:text-foreground"
          }`}
        >
          <span className="max-w-[90px] truncate">{tab.title}</span>
          <button
            onClick={(e) => { e.stopPropagation(); onClose(tab.id); }}
            className="ml-0.5 rounded p-0.5 hover:bg-border/30"
          >
            <X size={10} />
          </button>
        </div>
      ))}
    </div>
  );
}
