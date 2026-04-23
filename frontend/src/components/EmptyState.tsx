import { Inbox } from "lucide-react";
import type { ReactNode } from "react";

interface Props {
  title?: string;
  description?: string;
  children?: ReactNode;
  className?: string;
}

export default function EmptyState({ title = "暂无数据", description = "", children, className = "" }: Props) {
  return (
    <div className={`flex flex-col items-center justify-center gap-2 text-gray-500 py-8 ${className}`}>
      <Inbox size={32} className="text-gray-300" />
      <div className="text-sm font-medium text-gray-600">{title}</div>
      {description && <div className="text-xs text-gray-400">{description}</div>}
      {children}
    </div>
  );
}
