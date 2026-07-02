import { useEffect } from "react";
import { useStore } from "./store";
import Sidebar from "./components/Sidebar";
import TopBar from "./components/TopBar";
import NodePalette from "./components/NodePalette";
import Canvas from "./components/Canvas";
import NodeDrawer from "./components/NodeDrawer";
import ConsolePanel from "./components/ConsolePanel";
import CopilotPanel from "./components/CopilotPanel";
import Toasts from "./components/Toasts";

export default function App() {
  const init = useStore((s) => s.init);
  const openLab = useStore((s) => s.openLab);
  const graph = useStore((s) => s.graph);
  const caps = useStore((s) => s.capabilities);

  useEffect(() => {
    init().then(() => {
      // Support deep-linking to a lab via ?lab=<name>.
      const lab = new URLSearchParams(window.location.search).get("lab");
      if (lab) openLab(lab);
    });
  }, [init, openLab]);

  return (
    <div className="flex h-full flex-col">
      <TopBar />

      {caps && !caps.runtimeAvailable && (
        <div className="bg-amber-100 px-3 py-1.5 text-xs text-amber-800 dark:bg-amber-950 dark:text-amber-300">
          Design-only mode: container runtime unavailable
          {caps.reason ? ` (${caps.reason})` : ""}. You can still design topologies, export YAML and
          use the Copilot.
        </div>
      )}

      <div className="flex min-h-0 flex-1">
        <Sidebar />
        {graph && <NodePalette />}
        <div className="flex min-w-0 flex-1 flex-col">
          <Canvas />
          <ConsolePanel />
        </div>
        {graph && <NodeDrawer />}
        <CopilotPanel />
      </div>

      <Toasts />
    </div>
  );
}
