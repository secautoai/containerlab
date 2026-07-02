import { useRef, useState, useEffect } from "react";
import { X, Sparkles, Send, Check, Play, Loader2 } from "lucide-react";
import { useStore } from "../store";
import { api, type Graph } from "../api";

interface ChatMsg {
  role: "user" | "assistant";
  text: string;
  notes?: string[];
  proposed?: Graph;
  applied?: boolean;
  source?: string;
}

const EXAMPLES = [
  "3-node OSPF triangle with SR Linux",
  "leaf-spine fabric: 3 leaves, 2 spines (Arista)",
  "4 FRR routers in a ring, each with a Linux host",
  "add a linux host connected to r1",
  "assign IPs and configure OSPF",
  "explain this lab",
  "what's wrong with my lab?",
];

export default function CopilotPanel() {
  const open = useStore((s) => s.copilotOpen);
  const toggle = useStore((s) => s.toggleCopilot);
  const caps = useStore((s) => s.capabilities);
  const graph = useStore((s) => s.graph);
  const applyProposedGraph = useStore((s) => s.applyProposedGraph);
  const adoptGraph = useStore((s) => s.adoptGraph);

  const [messages, setMessages] = useState<ChatMsg[]>([]);
  const [input, setInput] = useState("");
  const [busy, setBusy] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    scrollRef.current?.scrollTo({ top: scrollRef.current.scrollHeight, behavior: "smooth" });
  }, [messages, busy]);

  if (!open) return null;

  const send = async (text: string) => {
    const message = text.trim();
    if (!message || busy) return;
    setInput("");
    setMessages((m) => [...m, { role: "user", text: message }]);
    setBusy(true);
    try {
      const reply = await api.aiChat(message, graph?.name);
      // Already-applied edits/config: adopt directly into the canvas.
      if (reply.applied && reply.proposedGraph) {
        adoptGraph(reply.proposedGraph);
      }
      setMessages((m) => [
        ...m,
        {
          role: "assistant",
          text: reply.reply,
          notes: reply.notes,
          proposed: reply.proposedGraph,
          applied: reply.applied,
          source: reply.source,
        },
      ]);
    } catch (e) {
      setMessages((m) => [...m, { role: "assistant", text: `Error: ${(e as Error).message}` }]);
    } finally {
      setBusy(false);
    }
  };

  return (
    <aside className="flex w-96 shrink-0 flex-col border-l border-slate-200 bg-white dark:border-slate-800 dark:bg-slate-900">
      <div className="flex items-center justify-between border-b border-slate-200 px-3 py-2 dark:border-slate-800">
        <span className="flex items-center gap-2 text-sm font-semibold">
          <Sparkles size={15} className="text-brand" /> Copilot
          <span className="rounded-full bg-slate-200 px-1.5 py-0.5 text-[10px] font-normal text-slate-500 dark:bg-slate-700 dark:text-slate-300">
            {caps?.aiAvailable ? "AI" : "offline"}
          </span>
        </span>
        <button onClick={() => toggle(false)} className="text-slate-400 hover:text-slate-600">
          <X size={16} />
        </button>
      </div>

      <div ref={scrollRef} className="flex-1 space-y-3 overflow-y-auto p-3 text-sm">
        {messages.length === 0 && (
          <div className="text-slate-400">
            <p className="mb-3">
              Describe the network you want and I'll design it. Try:
            </p>
            <div className="space-y-1.5">
              {EXAMPLES.map((ex) => (
                <button
                  key={ex}
                  onClick={() => send(ex)}
                  className="block w-full rounded-md border border-slate-200 bg-slate-50 px-2.5 py-1.5 text-left text-xs hover:border-brand dark:border-slate-700 dark:bg-slate-800"
                >
                  {ex}
                </button>
              ))}
            </div>
          </div>
        )}

        {messages.map((m, i) => (
          <div key={i} className={m.role === "user" ? "flex justify-end" : ""}>
            <div
              className={`max-w-[90%] rounded-lg px-3 py-2 ${
                m.role === "user"
                  ? "bg-brand text-slate-900"
                  : "bg-slate-100 text-slate-800 dark:bg-slate-800 dark:text-slate-100"
              }`}
            >
              <p className="whitespace-pre-wrap break-words">{m.text}</p>
              {m.notes && m.notes.length > 0 && (
                <ul className="mt-2 list-disc space-y-0.5 pl-4 text-xs text-slate-500 dark:text-slate-400">
                  {m.notes.map((n, j) => (
                    <li key={j}>{n}</li>
                  ))}
                </ul>
              )}
              {m.proposed && m.applied && (
                <div className="mt-2 text-xs text-emerald-500">
                  ✓ applied to canvas ({m.proposed.nodes?.length ?? 0} nodes,{" "}
                  {m.proposed.links?.length ?? 0} links)
                </div>
              )}
              {m.proposed && !m.applied && (
                <div className="mt-2 rounded-md border border-slate-300 bg-white p-2 text-xs dark:border-slate-600 dark:bg-slate-900">
                  <div className="mb-1 font-medium">
                    {m.proposed.name} · {m.proposed.nodes?.length ?? 0} nodes ·{" "}
                    {m.proposed.links?.length ?? 0} links
                  </div>
                  <div className="flex gap-1.5">
                    <button
                      onClick={() => applyProposedGraph(m.proposed!)}
                      className="flex items-center gap-1 rounded-md border border-slate-300 px-2 py-1 hover:border-brand dark:border-slate-600"
                    >
                      <Check size={12} /> Apply
                    </button>
                    <button
                      disabled={!caps?.runtimeAvailable}
                      onClick={() => applyProposedGraph(m.proposed!, true)}
                      className="flex items-center gap-1 rounded-md bg-emerald-500 px-2 py-1 text-white hover:bg-emerald-600 disabled:opacity-40"
                    >
                      <Play size={12} /> Apply & Deploy
                    </button>
                  </div>
                </div>
              )}
            </div>
          </div>
        ))}

        {busy && (
          <div className="flex items-center gap-2 text-slate-400">
            <Loader2 size={14} className="animate-spin" /> Thinking…
          </div>
        )}
      </div>

      <div className="border-t border-slate-200 p-2 dark:border-slate-800">
        <div className="flex items-end gap-2">
          <textarea
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                send(input);
              }
            }}
            rows={2}
            placeholder="Describe a network to build…"
            className="flex-1 resize-none rounded-md border border-slate-300 bg-slate-50 px-2 py-1.5 text-sm outline-none focus:border-brand dark:border-slate-700 dark:bg-slate-800"
          />
          <button
            onClick={() => send(input)}
            disabled={busy || !input.trim()}
            className="rounded-md bg-brand p-2 text-slate-900 hover:bg-brand-600 disabled:opacity-40"
          >
            <Send size={16} />
          </button>
        </div>
      </div>
    </aside>
  );
}
