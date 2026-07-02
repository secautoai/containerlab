import { useEffect, useRef } from "react";
import { X } from "lucide-react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";
import { useStore } from "../store";

// ConsolePanel opens an interactive terminal into a running node via a
// WebSocket bridged to a docker exec TTY on the backend.
export default function ConsolePanel() {
  const consoleNode = useStore((s) => s.consoleNode);
  const graph = useStore((s) => s.graph);
  const openConsole = useStore((s) => s.openConsole);
  const containerRef = useRef<HTMLDivElement>(null);

  const labName = graph?.name;

  useEffect(() => {
    if (!consoleNode || !labName || !containerRef.current) return;

    const term = new Terminal({
      convertEol: true,
      cursorBlink: true,
      fontFamily: "JetBrains Mono, ui-monospace, monospace",
      fontSize: 13,
      theme: { background: "#000000" },
    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.open(containerRef.current);
    fit.fit();

    const proto = window.location.protocol === "https:" ? "wss" : "ws";
    const url = `${proto}://${window.location.host}/api/labs/${encodeURIComponent(
      labName,
    )}/nodes/${encodeURIComponent(consoleNode)}/console`;
    const ws = new WebSocket(url);
    ws.binaryType = "arraybuffer";

    const sendResize = () => {
      fit.fit();
      if (ws.readyState === WebSocket.OPEN) {
        ws.send(JSON.stringify({ type: "resize", cols: term.cols, rows: term.rows }));
      }
    };

    ws.onopen = () => {
      term.writeln("\x1b[90mConnecting…\x1b[0m");
      sendResize();
    };
    ws.onmessage = (ev) => {
      if (typeof ev.data === "string") {
        term.write(ev.data);
      } else {
        term.write(new Uint8Array(ev.data));
      }
    };
    ws.onclose = () => term.writeln("\r\n\x1b[90m[connection closed]\x1b[0m");
    ws.onerror = () => term.writeln("\r\n\x1b[31m[connection error]\x1b[0m");

    const disposable = term.onData((data) => {
      if (ws.readyState === WebSocket.OPEN) ws.send(data);
    });

    const onWindowResize = () => sendResize();
    window.addEventListener("resize", onWindowResize);

    return () => {
      window.removeEventListener("resize", onWindowResize);
      disposable.dispose();
      ws.close();
      term.dispose();
    };
  }, [consoleNode, labName]);

  if (!consoleNode) return null;

  return (
    <div className="flex h-64 shrink-0 flex-col border-t border-slate-300 bg-black dark:border-slate-800">
      <div className="flex items-center justify-between border-b border-slate-800 bg-slate-900 px-3 py-1.5 text-xs text-slate-200">
        <span className="font-mono">console: {consoleNode}</span>
        <button onClick={() => openConsole(undefined)} className="text-slate-400 hover:text-white">
          <X size={14} />
        </button>
      </div>
      <div ref={containerRef} className="min-h-0 flex-1 overflow-hidden p-1" />
    </div>
  );
}
