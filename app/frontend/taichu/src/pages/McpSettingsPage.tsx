import { useEffect, useMemo, useState } from "react";
import { Cloud, Database, FileText, FolderX, Loader2, Plus, RefreshCw, Search, Server, Settings2, Trash2, Wrench, X } from "lucide-react";
import { apiClient } from "../api/client";
import { message } from "../components/message";
import type { McpConfigView, McpDiscoveryReport, McpServerAdminView, McpServerConfig, McpTransport } from "../types/configPages";
import "./McpSettingsPage.css";

type McpFormState = { originalName?: string; name: string; transport: McpTransport; command: string; args: string; envRows: Array<{ key: string; value: string }>; url: string; timeout: number };

function emptyForm(): McpFormState { return { name: "local-server", transport: "stdio", command: "python3", args: "server.py", envRows: [{ key: "", value: "" }], url: "https://mcp.example.com/sse", timeout: 30000 }; }
function mapToRows(value?: Record<string, string>) { const rows = Object.entries(value || {}).map(([key, item]) => ({ key, value: item })); return rows.length ? rows : [{ key: "", value: "" }]; }
function rowsToMap(rows: Array<{ key: string; value: string }>) { return rows.reduce<Record<string, string>>((next, row) => { if (row.key.trim()) next[row.key.trim()] = row.value; return next; }, {}); }
function argsToArray(value: string) { return value.split(/\s+/).map((item) => item.trim()).filter(Boolean); }
function formFromServer(server: McpServerAdminView): McpFormState { const config = server.config; if (config.type === "stdio") return { originalName: server.name, name: server.name, transport: "stdio", command: config.command, args: (config.args || []).join(" "), envRows: mapToRows(config.env), url: "", timeout: config.tool_call_timeout_ms || 30000 }; return { originalName: server.name, name: server.name, transport: config.type, command: "", args: "", envRows: mapToRows(config.headers), url: config.url, timeout: 30000 }; }
function configFromForm(form: McpFormState): McpServerConfig { if (form.transport === "stdio") return { type: "stdio", command: form.command, args: argsToArray(form.args), env: rowsToMap(form.envRows), tool_call_timeout_ms: Number(form.timeout) || 30000 }; return { type: form.transport, url: form.url, headers: rowsToMap(form.envRows) }; }
function isHealthy(server: McpServerAdminView) { return server.status === "resolved" || server.status === "configured"; }

export function McpSettingsPage() {
  const [config, setConfig] = useState<McpConfigView | null>(null);
  const [servers, setServers] = useState<McpServerAdminView[]>([]);
  const [form, setForm] = useState<McpFormState>(emptyForm);
  const [sheetOpen, setSheetOpen] = useState(false);
  const [sheetTitle, setSheetTitle] = useState("Add MCP Server");
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [discovering, setDiscovering] = useState(false);
  const [search, setSearch] = useState("");

  async function loadServers(nextName?: string) {
    setLoading(true);
    try {
      const [nextConfig, nextServers] = await Promise.all([apiClient.get<McpConfigView>("/yuanling/mcp/config"), apiClient.get<McpServerAdminView[]>("/yuanling/mcp/servers")]);
      setConfig(nextConfig); setServers(nextServers);
      const target = nextServers.find((item) => item.name === nextName);
      if (target) setForm(formFromServer(target));
    } finally { setLoading(false); }
  }

  useEffect(() => { void loadServers(); }, []);

  const filteredServers = useMemo(() => { const query = search.trim().toLowerCase(); if (!query) return servers; return servers.filter((server) => [server.name, server.config.type, server.status, server.detail || ""].join(" ").toLowerCase().includes(query)); }, [search, servers]);

  function openSheet(title: string, server?: McpServerAdminView) { setSheetTitle(title); setForm(server ? formFromServer(server) : emptyForm()); setSheetOpen(true); }
  function updateEnvRow(index: number, field: "key" | "value", value: string) { setForm((current) => ({ ...current, envRows: current.envRows.map((row, rowIndex) => rowIndex === index ? { ...row, [field]: value } : row) })); }
  function removeEnvRow(index: number) { setForm((current) => ({ ...current, envRows: current.envRows.filter((_, rowIndex) => rowIndex !== index).length ? current.envRows.filter((_, rowIndex) => rowIndex !== index) : [{ key: "", value: "" }] })); }

  async function saveServer() { setSaving(true); try { const payload = { name: form.name, config: configFromForm(form) }; const saved = form.originalName ? await apiClient.put<McpServerAdminView>(`/yuanling/mcp/servers/${encodeURIComponent(form.originalName)}`, payload) : await apiClient.post<McpServerAdminView>("/yuanling/mcp/servers", payload); setSheetOpen(false); message.info("Server Saved", "Connecting to the MCP Server..."); await loadServers(saved.name); } finally { setSaving(false); } }
  async function deleteServer(server: McpServerAdminView) { const ok = await message.confirm({ title: `Remove "${server.name}"?`, desc: "This will disconnect the MCP server from local page configuration.", confirmText: "Remove Server", destructive: true }); if (!ok) return; await apiClient.delete<string>(`/yuanling/mcp/servers/${encodeURIComponent(server.name)}`); message.success("Server Removed", `${server.name} has been disconnected.`); await loadServers(); }
  async function discoverTools() { setDiscovering(true); try { const report = await apiClient.post<McpDiscoveryReport>("/yuanling/mcp/discover"); message.info("Syncing", `${report.tools.length} tools discovered, ${report.failed_servers.length} servers failed.`); await loadServers(); } finally { setDiscovering(false); } }

  return (
    <main className="main-content mcp-prototype">
      <header className="page-header">
        <div className="header-title"><h1>MCP Servers</h1><p>Connect secure data sources and tools via Model Context Protocol.</p></div>
        <div className="header-actions">
          <button className="btn btn-default" type="button" disabled={discovering} onClick={() => void discoverTools()}>{discovering ? <Loader2 className="spin" size={16} /> : <RefreshCw size={16} />} Refresh All</button>
          <button className="btn btn-primary" type="button" onClick={() => openSheet("Add MCP Server")}><Plus size={16} /> Add Server</button>
        </div>
      </header>

      <div className="toolbar"><div className="search-input-box"><Search size={16} /><input className="search-input" value={search} onChange={(event) => setSearch(event.target.value)} placeholder="Search servers..." /></div></div>

      <div className="list-container">
        {loading ? <div className="empty-row"><Loader2 className="spin" size={18} /> Loading MCP servers...</div> : filteredServers.length ? filteredServers.map((server) => {
          const healthy = isHealthy(server); const Icon = server.config.type === "stdio" ? Database : server.config.type === "sse" ? Cloud : healthy ? Server : FolderX;
          return <div key={server.name} className="list-item" data-error={!healthy}>
            <div className="item-icon"><Icon size={20} /></div>
            <div className="item-info">
              <div className="item-title-row"><span className="item-title">{server.name}</span><span className={`badge ${server.config.type}`}>{server.config.type}</span><div className="status-indicator"><div className={`status-dot ${healthy ? "connected" : "error"}`} />{healthy ? "Connected" : "Connection Error"}</div></div>
              <div className="item-command">{server.config.type === "stdio" ? [server.config.command, ...(server.config.args || [])].join(" ") : server.config.url}</div>
              <div className="item-meta"><span><Wrench size={12} /> {config?.configured_count || 0} Servers</span><span><FileText size={12} /> {server.detail || server.status}</span></div>
            </div>
            <div className="item-actions"><button className="btn-icon spin" type="button" title="Reload Server" onClick={() => void discoverTools()}><RefreshCw size={16} /></button><button className="btn-icon" type="button" title="Settings" onClick={() => openSheet("Edit Server", server)}><Settings2 size={16} /></button><button className="btn-icon danger" type="button" onClick={() => void deleteServer(server)}><Trash2 size={16} /></button></div>
          </div>;
        }) : <div className="empty-row">No MCP servers configured.</div>}
      </div>

      <div className={`overlay${sheetOpen ? " show" : ""}`} onClick={() => setSheetOpen(false)} />
      <div className={`sheet${sheetOpen ? " open" : ""}`}>
        <div className="sheet-header"><h2 className="sheet-title">{sheetTitle}</h2><button className="sheet-close" type="button" onClick={() => setSheetOpen(false)}><X size={20} /></button></div>
        <div className="sheet-body">
          <div className="form-group"><label>Server Name</label><input className="input-field" value={form.name} onChange={(event) => setForm({ ...form, name: event.target.value })} placeholder="e.g. My Database MCP" /></div>
          <div className="form-group"><label>Transport Type</label><select className="select-field" value={form.transport} onChange={(event) => setForm({ ...form, transport: event.target.value as McpTransport })}><option value="stdio">stdio (Local Command)</option><option value="sse">sse (Remote Server-Sent Events)</option><option value="http">http (Remote HTTP)</option><option value="ws">ws (WebSocket)</option></select></div>
          {form.transport === "stdio" ? <div className="transport-fields"><div className="form-group"><label>Command</label><input className="input-field mono" value={form.command} onChange={(event) => setForm({ ...form, command: event.target.value })} placeholder="e.g. npx, node, python" /></div><div className="form-group"><label>Arguments (Space separated)</label><input className="input-field mono" value={form.args} onChange={(event) => setForm({ ...form, args: event.target.value })} placeholder="-y @modelcontextprotocol/server-postgres" /></div><div className="form-group"><label>Tool Timeout Ms</label><input className="input-field mono" type="number" value={form.timeout} onChange={(event) => setForm({ ...form, timeout: Number(event.target.value) })} /></div></div> : <div className="transport-fields"><div className="form-group"><label>Endpoint URL</label><input className="input-field mono" value={form.url} onChange={(event) => setForm({ ...form, url: event.target.value })} placeholder="https://mcp.example.com/sse" /></div></div>}
          <hr />
          <div className="form-group"><div className="env-header"><label>{form.transport === "stdio" ? "Environment Variables" : "Headers"}</label><button className="btn btn-default mini" type="button" onClick={() => setForm({ ...form, envRows: [...form.envRows, { key: "", value: "" }] })}><Plus size={12} /> Add</button></div><div className="env-list">{form.envRows.map((row, index) => <div className="env-row" key={`${index}-${row.key}`}><input className="env-input" value={row.key} onChange={(event) => updateEnvRow(index, "key", event.target.value)} placeholder="Key" /><input className="env-input" value={row.value} onChange={(event) => updateEnvRow(index, "value", event.target.value)} placeholder="Value" /><button className="env-action" type="button" onClick={() => removeEnvRow(index)}><Trash2 size={14} /></button></div>)}</div></div>
        </div>
        <div className="sheet-footer"><button className="btn btn-default" type="button" onClick={() => setSheetOpen(false)}>Cancel</button><button className="btn btn-primary" type="button" disabled={saving} onClick={() => void saveServer()}>{saving ? "Saving..." : "Save & Connect"}</button></div>
      </div>
    </main>
  );
}
