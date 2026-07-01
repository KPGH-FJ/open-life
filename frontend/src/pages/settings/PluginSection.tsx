import { useState } from "react";
import { reloadPlugins, enablePlugin, disablePlugin } from "../../tauri";
import type { PluginRecord } from "../../tauri";

interface PluginSectionProps {
  plugins: PluginRecord[];
  onPluginsChange: (plugins: PluginRecord[]) => void;
  onRefreshDiagnostics: () => void;
}

export default function PluginSection({
  plugins,
  onPluginsChange,
  onRefreshDiagnostics,
}: PluginSectionProps) {
  const [loading, setLoading] = useState(false);

  const handleReload = async () => {
    setLoading(true);
    try {
      const records = await reloadPlugins();
      onPluginsChange(records);
    } finally {
      setLoading(false);
    }
  };

  const handleToggle = async (plugin: PluginRecord) => {
    try {
      if (plugin.enabled) {
        await disablePlugin(plugin.manifest.id);
      } else {
        await enablePlugin(plugin.manifest.id);
      }
      await onRefreshDiagnostics();
    } catch (e) {
      console.error("Plugin toggle failed:", e);
    }
  };

  return (
    <section className="space-y-4 border-t pt-4">
      <div className="flex items-center justify-between gap-3">
        <div>
          <h3 className="text-sm font-medium text-gray-700">本地 Plugins</h3>
          <p className="mt-1 text-xs text-gray-500">
            预览能力：Plugin 当前仅做声明展示，tools 不可执行，skills 仅注册声明，不执行远程代码。
          </p>
        </div>
        <button
          onClick={handleReload}
          disabled={loading}
          className="rounded-md border border-gray-300 bg-white px-3 py-1.5 text-xs font-medium text-gray-700 hover:bg-gray-50 disabled:opacity-50"
        >
          {loading ? "加载中..." : "重新加载"}
        </button>
      </div>
      <div className="space-y-2">
        {plugins.length === 0 ? (
          <div className="rounded-lg border border-dashed border-gray-200 bg-gray-50 px-3 py-3 text-xs text-gray-500">
            暂未发现本地 plugin manifest。
          </div>
        ) : (
          plugins.map(plugin => (
            <div
              key={plugin.manifest.id}
              className="rounded-lg border border-gray-200 bg-white px-3 py-3"
            >
              <div className="flex items-center justify-between gap-3">
                <div className="min-w-0">
                  <div className="text-sm font-medium text-gray-900">{plugin.manifest.name}</div>
                  <div className="mt-1 text-xs text-gray-500">
                    {plugin.manifest.id} · v{plugin.manifest.version} ·{" "}
                    {plugin.enabled ? "enabled" : "disabled"}
                  </div>
                </div>
                <button
                  onClick={() => handleToggle(plugin)}
                  disabled={Boolean(plugin.error)}
                  className="shrink-0 rounded-md border border-gray-300 bg-white px-3 py-1.5 text-xs font-medium text-gray-700 hover:bg-gray-50 disabled:opacity-50"
                >
                  {plugin.enabled ? "禁用" : "启用"}
                </button>
              </div>
              {plugin.error && (
                <div className="mt-2 rounded-md bg-rose-50 px-2 py-1.5 text-xs text-rose-700">
                  {plugin.error}
                </div>
              )}
              {plugin.enabled && !plugin.error && (
                <div className="mt-2 space-y-1">
                  {plugin.manifest.tools.length > 0 && (
                    <div className="text-xs text-gray-500">
                      <span className="font-medium">Tools (声明):</span>{" "}
                      {plugin.manifest.tools.map(t => t.name).join(", ")}
                      <span className="ml-1 text-amber-600">[暂不可执行]</span>
                    </div>
                  )}
                  {plugin.manifest.skills.length > 0 && (
                    <div className="text-xs text-gray-500">
                      <span className="font-medium">Skills:</span>{" "}
                      {plugin.manifest.skills.map(s => s.name).join(", ")}
                    </div>
                  )}
                </div>
              )}
            </div>
          ))
        )}
      </div>
    </section>
  );
}
