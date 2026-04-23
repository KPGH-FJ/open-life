import { Loader2 } from "lucide-react";

interface Props {
  text?: string;
  className?: string;
}

export default function LoadingSpinner({ text = "加载中...", className = "" }: Props) {
  return (
    <div className={`flex flex-col items-center justify-center gap-2 text-gray-500 ${className}`}>
      <Loader2 size={24} className="animate-spin text-indigo-600" />
      <span className="text-sm">{text}</span>
    </div>
  );
}
