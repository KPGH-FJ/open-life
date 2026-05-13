import { useState } from "react";

export interface StateRecordData {
  name: string;
  value: number;
  unit: string;
  note?: string;
  minThreshold?: number;
  maxThreshold?: number;
  alertDays?: number;
}

interface StateRecordModalProps {
  show: boolean;
  onClose: () => void;
  onSave: (data: StateRecordData) => Promise<void>;
}

export default function StateRecordModal({ show, onClose, onSave }: StateRecordModalProps) {
  const [name, setName] = useState("");
  const [value, setValue] = useState("");
  const [unit, setUnit] = useState("");
  const [note, setNote] = useState("");
  const [minThreshold, setMinThreshold] = useState("");
  const [maxThreshold, setMaxThreshold] = useState("");
  const [alertDays, setAlertDays] = useState("3");

  if (!show) return null;

  const handleSave = async () => {
    const trimmedName = name.trim();
    const parsedValue = parseFloat(value);
    if (!trimmedName || Number.isNaN(parsedValue)) return;

    await onSave({
      name: trimmedName,
      value: parsedValue,
      unit: unit.trim() || "单位",
      note: note.trim() || undefined,
      minThreshold: minThreshold.trim() ? parseFloat(minThreshold.trim()) : undefined,
      maxThreshold: maxThreshold.trim() ? parseFloat(maxThreshold.trim()) : undefined,
      alertDays: alertDays.trim() ? parseInt(alertDays.trim(), 10) : undefined,
    });

    setName("");
    setValue("");
    setUnit("");
    setNote("");
    setMinThreshold("");
    setMaxThreshold("");
    setAlertDays("3");
    onClose();
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4">
      <div className="bg-white rounded-xl w-full max-w-md p-5 shadow-lg max-h-[90vh] overflow-auto">
        <div className="font-semibold text-gray-800 mb-4">记录状态</div>
        <div className="space-y-3">
          <div>
            <label className="block text-sm text-gray-600 mb-1">维度名称</label>
            <input
              value={name}
              onChange={e => setName(e.target.value)}
              placeholder="例如：体重、睡眠时长、专注度"
              className="w-full border rounded-lg px-3 py-2 text-sm"
            />
          </div>
          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="block text-sm text-gray-600 mb-1">数值</label>
              <input
                type="number"
                step="0.1"
                value={value}
                onChange={e => setValue(e.target.value)}
                placeholder="0.0"
                className="w-full border rounded-lg px-3 py-2 text-sm"
              />
            </div>
            <div>
              <label className="block text-sm text-gray-600 mb-1">单位</label>
              <input
                value={unit}
                onChange={e => setUnit(e.target.value)}
                placeholder="kg / h / 分"
                className="w-full border rounded-lg px-3 py-2 text-sm"
              />
            </div>
          </div>
          <div className="grid grid-cols-3 gap-3">
            <div>
              <label className="block text-sm text-gray-600 mb-1">最小阈值</label>
              <input
                type="number"
                step="0.1"
                value={minThreshold}
                onChange={e => setMinThreshold(e.target.value)}
                placeholder="可选"
                className="w-full border rounded-lg px-3 py-2 text-sm"
              />
            </div>
            <div>
              <label className="block text-sm text-gray-600 mb-1">最大阈值</label>
              <input
                type="number"
                step="0.1"
                value={maxThreshold}
                onChange={e => setMaxThreshold(e.target.value)}
                placeholder="可选"
                className="w-full border rounded-lg px-3 py-2 text-sm"
              />
            </div>
            <div>
              <label className="block text-sm text-gray-600 mb-1">预警天数</label>
              <input
                type="number"
                min={1}
                value={alertDays}
                onChange={e => setAlertDays(e.target.value)}
                className="w-full border rounded-lg px-3 py-2 text-sm"
              />
            </div>
          </div>
          <div>
            <label className="block text-sm text-gray-600 mb-1">备注（可选）</label>
            <input
              value={note}
              onChange={e => setNote(e.target.value)}
              placeholder="今天感觉如何？"
              className="w-full border rounded-lg px-3 py-2 text-sm"
            />
          </div>
        </div>
        <div className="flex justify-end gap-2 mt-5">
          <button
            onClick={onClose}
            className="px-4 py-2 text-sm text-gray-600 hover:bg-gray-100 rounded-lg"
          >
            取消
          </button>
          <button
            onClick={handleSave}
            className="px-4 py-2 text-sm bg-indigo-600 text-white rounded-lg hover:bg-indigo-700"
          >
            保存
          </button>
        </div>
      </div>
    </div>
  );
}
