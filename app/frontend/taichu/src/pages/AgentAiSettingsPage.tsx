import { useEffect, useMemo, useState } from "react";
import {
  BarChart3,
  Code,
  Eye,
  EyeOff,
  Key,
  Link,
  Loader2,
  PenTool,
  Play,
  Plus,
  Save,
  Search,
  Trash2,
} from "lucide-react";
import { apiClient } from "../api/client";
import { message } from "../components/message";
import type {
  AiInstanceFormState,
  AiInstancePayload,
  AiInstanceTestResult,
  AiInstanceView,
  AiProvider,
} from "../types/ai";
import "./AgentAiSettingsPage.css";

const draftInstanceId = "__draft__";

const providerDefaults: Record<AiProvider, { base_url: string; request_path: string; auth_header: string; model: string }> = {
  "openai-compatible": {
    base_url: "https://api.openai.com/v1",
    request_path: "/chat/completions",
    auth_header: "Authorization",
    model: "gpt-4o-mini",
  },
  "anthropic-compatible": {
    base_url: "https://api.anthropic.com",
    request_path: "/v1/messages",
    auth_header: "x-api-key",
    model: "claude-3-5-sonnet-20240620",
  },
  custom: {
    base_url: "http://localhost:8000/v1",
    request_path: "/chat/completions",
    auth_header: "Authorization",
    model: "local-default",
  },
};

function createEmptyForm(): AiInstanceFormState {
  const defaults = providerDefaults["openai-compatible"];
  return {
    name: "Untitled",
    enabled: true,
    provider: "openai-compatible",
    base_url: defaults.base_url,
    request_path: defaults.request_path,
    api_key: "",
    model: defaults.model,
    prompt_template: "You are a world-class assistant. Be clear, practical, and concise.",
    timeout_ms: 60000,
    auth_header: defaults.auth_header,
    stream: false,
    max_tokens: 2048,
    temperature: 0.7,
    top_p: 0.95,
    frequency_penalty: undefined,
    presence_penalty: undefined,
    stop: undefined,
    reasoning_effort: undefined,
    stop_text: "",
  };
}

function formFromInstance(instance: AiInstanceView): AiInstanceFormState {
  return {
    id: instance.id,
    name: instance.name,
    enabled: instance.enabled,
    provider: instance.provider,
    base_url: instance.base_url,
    request_path: instance.request_path,
    api_key: "",
    model: instance.model,
    prompt_template: instance.prompt_template,
    timeout_ms: instance.timeout_ms,
    auth_header: instance.auth_header,
    stream: instance.stream,
    max_tokens: instance.max_tokens,
    temperature: instance.temperature,
    top_p: instance.top_p,
    frequency_penalty: instance.frequency_penalty,
    presence_penalty: instance.presence_penalty,
    stop: instance.stop,
    reasoning_effort: instance.reasoning_effort,
    stop_text: instance.stop?.join("\n") || "",
  };
}

function optionalNumber(value: number | undefined) {
  return Number.isFinite(value) ? value : undefined;
}

function payloadFromForm(form: AiInstanceFormState): AiInstancePayload {
  const stop = form.stop_text
    .split(/[\n,]/)
    .map((item) => item.trim())
    .filter(Boolean);
  return {
    name: form.name,
    enabled: form.enabled,
    provider: form.provider,
    base_url: form.base_url,
    request_path: form.request_path,
    api_key: form.api_key?.trim() || undefined,
    model: form.model,
    prompt_template: form.prompt_template,
    timeout_ms: Number(form.timeout_ms) || 60000,
    auth_header: form.auth_header,
    stream: form.stream,
    max_tokens: form.max_tokens ? Number(form.max_tokens) : undefined,
    temperature: optionalNumber(form.temperature),
    top_p: optionalNumber(form.top_p),
    frequency_penalty: optionalNumber(form.frequency_penalty),
    presence_penalty: optionalNumber(form.presence_penalty),
    stop: stop.length ? stop : undefined,
    reasoning_effort: form.reasoning_effort || undefined,
  };
}

function providerLabel(provider: AiProvider) {
  if (provider === "anthropic-compatible") return "Anthropic";
  if (provider === "custom") return "Custom";
  return "OpenAI";
}

function providerAvatar(provider: AiProvider) {
  if (provider === "anthropic-compatible") return { Icon: Code, color: "#166534" };
  if (provider === "custom") return { Icon: BarChart3, color: "#7c2d12" };
  return { Icon: PenTool, color: "#0f172a" };
}

function extractAssistantText(body?: string) {
  if (!body) return "";
  try {
    const payload = JSON.parse(body) as {
      choices?: Array<{ message?: { content?: string } }>;
      content?: Array<{ type?: string; text?: string }>;
    };
    const openAiText = payload.choices?.[0]?.message?.content;
    if (openAiText) return openAiText;
    const anthropicText = payload.content
      ?.filter((block) => block.type === "text" && block.text)
      .map((block) => block.text)
      .join("\n");
    return anthropicText || body;
  } catch {
    return body;
  }
}

export function AgentAiSettingsPage() {
  const [instances, setInstances] = useState<AiInstanceView[]>([]);
  const [draftInstance, setDraftInstance] = useState<AiInstanceView | null>(null);
  const [selectedId, setSelectedId] = useState<string | undefined>();
  const [form, setForm] = useState<AiInstanceFormState>(createEmptyForm);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);
  const [search, setSearch] = useState("");
  const [showApiKey, setShowApiKey] = useState(false);
  const [testMessage, setTestMessage] = useState("Please reply with one short sentence confirming this AI instance is working.");
  const [testReply, setTestReply] = useState("");

  const selected = useMemo(
    () => (selectedId === draftInstanceId ? draftInstance : instances.find((item) => item.id === selectedId)),
    [draftInstance, instances, selectedId],
  );
  const visibleInstances = useMemo(
    () => (draftInstance ? [draftInstance, ...instances] : instances),
    [draftInstance, instances],
  );
  const filteredInstances = useMemo(() => {
    const query = search.trim().toLowerCase();
    if (!query) return visibleInstances;
    return visibleInstances.filter((instance) =>
      [instance.name, instance.provider, instance.model]
        .join(" ")
        .toLowerCase()
        .includes(query),
    );
  }, [search, visibleInstances]);

  async function loadInstances(nextSelectedId?: string) {
    setLoading(true);
    try {
      const data = await apiClient.get<AiInstanceView[]>("/yuanling/ai/instances");
      setInstances(data);
      const target = nextSelectedId ? data.find((item) => item.id === nextSelectedId) : data[0];
      setSelectedId(target?.id);
      setForm(target ? formFromInstance(target) : createEmptyForm());
    } catch (error) {
      console.error(error);
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void loadInstances();
  }, []);

  function selectInstance(instance: AiInstanceView) {
    setSelectedId(instance.id);
    setForm(formFromInstance(instance));
    setShowApiKey(false);
    setTestReply("");
  }

  function createDraft() {
    const draftForm = createEmptyForm();
    const now = Date.now();
    const draft: AiInstanceView = {
      id: draftInstanceId,
      name: draftForm.name,
      enabled: draftForm.enabled,
      provider: draftForm.provider,
      display_name: "OpenAI Compatible",
      base_url: draftForm.base_url,
      request_path: draftForm.request_path,
      request_url: `${draftForm.base_url}${draftForm.request_path}`,
      model: draftForm.model,
      prompt_template: draftForm.prompt_template,
      timeout_ms: draftForm.timeout_ms,
      auth_header: draftForm.auth_header,
      has_api_key: false,
      stream: draftForm.stream,
      max_tokens: draftForm.max_tokens,
      temperature: draftForm.temperature,
      top_p: draftForm.top_p,
      frequency_penalty: draftForm.frequency_penalty,
      presence_penalty: draftForm.presence_penalty,
      stop: draftForm.stop,
      reasoning_effort: draftForm.reasoning_effort,
      supported_params: {
        model: true,
        max_tokens: true,
        messages: true,
        system: true,
        stream: true,
        tools: true,
        tool_choice: true,
        temperature: true,
        top_p: true,
        frequency_penalty: true,
        presence_penalty: true,
        stop: true,
        reasoning_effort: true,
      },
      created_at_ms: now,
      updated_at_ms: now,
    };
    setDraftInstance(draft);
    setSelectedId(draftInstanceId);
    setForm(draftForm);
    setShowApiKey(false);
    setTestReply("");
    message.info("New AI draft created", "Untitled 已添加到左侧列表，保存后会成为正式 AI 实例。");
  }

  function updateProvider(provider: AiProvider) {
    const defaults = providerDefaults[provider];
    updateForm({
      ...form,
      provider,
      base_url: form.base_url || defaults.base_url,
      request_path: !form.request_path || Object.values(providerDefaults).some((item) => item.request_path === form.request_path)
        ? defaults.request_path
        : form.request_path,
      auth_header: !form.auth_header || Object.values(providerDefaults).some((item) => item.auth_header === form.auth_header)
        ? defaults.auth_header
        : form.auth_header,
      model: form.model || defaults.model,
    });
  }

  async function saveInstance() {
    setSaving(true);
    try {
      const payload = payloadFromForm(form);
      const saved = form.id && form.id !== draftInstanceId
        ? await apiClient.put<AiInstanceView>(`/yuanling/ai/instances/${form.id}`, payload)
        : await apiClient.post<AiInstanceView>("/yuanling/ai/instances", payload);
      setDraftInstance(null);
      message.success("AI instance saved", `${saved.name} 已成功保存。`);
      setTestReply("");
      await loadInstances(saved.id);
    } catch (error) {
      console.error(error);
    } finally {
      setSaving(false);
    }
  }

  async function deleteInstance() {
    if (selectedId === draftInstanceId) {
      setDraftInstance(null);
      setSelectedId(instances[0]?.id);
      setForm(instances[0] ? formFromInstance(instances[0]) : createEmptyForm());
      message.info("Draft removed", "Untitled 草稿已移除。");
      return;
    }
    if (!form.id) return;
    setSaving(true);
    try {
      await apiClient.delete<string>(`/yuanling/ai/instances/${form.id}`);
      message.success("AI instance deleted", `${form.name} 已删除。`);
      await loadInstances();
    } catch (error) {
      console.error(error);
    } finally {
      setSaving(false);
    }
  }

  async function testConnection() {
    if (!form.id || form.id === draftInstanceId) {
      message.warn("Please save first", "请先保存这个 AI 实例，再运行 Test Run。");
      return;
    }
    setTesting(true);
    try {
      const result = await apiClient.post<AiInstanceTestResult>(`/yuanling/ai/instances/${form.id}/test`, {
        message: testMessage,
        max_tokens: form.max_tokens || 128,
      });
      const reply = extractAssistantText(result.result.body);
      setTestReply(reply);
      if (result.result.success) {
        message.success("Test run succeeded", `HTTP ${result.result.status ?? "OK"}，AI 已返回回复。`);
      } else {
        message.error("Test run failed", result.result.error || "连接测试失败");
      }
    } catch (error) {
      console.error(error);
    } finally {
      setTesting(false);
    }
  }

  function updateForm(nextForm: AiInstanceFormState) {
    setForm(nextForm);
    if (selectedId !== draftInstanceId || !draftInstance) return;
    setDraftInstance({
      ...draftInstance,
      name: nextForm.name || "Untitled",
      enabled: nextForm.enabled,
      provider: nextForm.provider,
      display_name: providerLabel(nextForm.provider),
      base_url: nextForm.base_url,
      request_path: nextForm.request_path,
      request_url: `${nextForm.base_url.replace(/\/$/, "")}/${nextForm.request_path.replace(/^\//, "")}`,
      model: nextForm.model,
      prompt_template: nextForm.prompt_template,
      timeout_ms: nextForm.timeout_ms,
      auth_header: nextForm.auth_header,
      stream: nextForm.stream,
      max_tokens: nextForm.max_tokens,
      temperature: nextForm.temperature,
      top_p: nextForm.top_p,
      frequency_penalty: nextForm.frequency_penalty,
      presence_penalty: nextForm.presence_penalty,
      stop: nextForm.stop,
      reasoning_effort: nextForm.reasoning_effort,
      updated_at_ms: Date.now(),
    });
  }

  return (
    <>
      <aside className="sidebar-secondary agent-config-sidebar">
        <div className="task-header agent-sidebar-header">
          <h2>My AI Clusters</h2>
          <button className="btn-ghost-sm" type="button" onClick={createDraft}>
            <Plus size={14} />
            New
          </button>
        </div>

        <div className="search-container">
          <div className="search-box">
            <Search size={16} color="var(--muted-foreground)" />
            <input value={search} type="text" placeholder="Search Agents..." onChange={(event) => setSearch(event.target.value)} />
          </div>
        </div>

        <div className="agent-list">
          {loading ? (
            <div className="agent-empty-state"><Loader2 size={16} className="spin" /> Loading agents...</div>
          ) : filteredInstances.length === 0 ? (
            <div className="agent-empty-state">No AI agents yet. Click New to create one.</div>
          ) : filteredInstances.map((instance) => {
            const avatar = providerAvatar(instance.provider);
            return (
              <button
                key={instance.id}
                type="button"
                className={`agent-item${instance.id === selectedId ? " active" : ""}`}
                onClick={() => selectInstance(instance)}
              >
                <div className="agent-avatar" style={{ background: avatar.color }}>
                  <avatar.Icon size={16} />
                </div>
                <div className="agent-info">
                  <div className="agent-name">{instance.name}</div>
                  <div className="agent-model">
                    <div className="badge">{providerLabel(instance.provider)}</div>
                    {instance.model}
                  </div>
                </div>
              </button>
            );
          })}
        </div>
      </aside>

      <main className="agent-editor-main">
        <header className="editor-header">
            <input
              className="editor-title"
              value={form.name}
              spellCheck={false}
              onChange={(event) => updateForm({ ...form, name: event.target.value })}
            />
          <div className="editor-actions">
            <button className="btn-secondary" type="button" onClick={testConnection} disabled={!form.id || testing}>
              {testing ? <Loader2 size={16} className="spin" /> : <Play size={16} />}
              Test Run
            </button>
            <button className="btn-primary" type="button" onClick={saveInstance} disabled={saving}>
              {saving ? <Loader2 size={16} className="spin" /> : <Save size={16} />}
              Save Changes
            </button>
          </div>
        </header>

        <div className="settings-container">
          <section className="card">
            <div className="card-header">
              <h2 className="card-title">Identity</h2>
              <p className="card-desc">Basic information to identify this AI instance.</p>
            </div>
            <div className="form-group">
              <label>Agent Name</label>
              <input className="input-field" value={form.name} placeholder="e.g. Marketing Expert" onChange={(event) => updateForm({ ...form, name: event.target.value })} />
            </div>
            <div className="form-row">
              <div className="form-group">
                <label>Status</label>
                <select className="select-field" value={form.enabled ? "true" : "false"} onChange={(event) => updateForm({ ...form, enabled: event.target.value === "true" })}>
                  <option value="true">Enabled</option>
                  <option value="false">Disabled</option>
                </select>
              </div>
              <div className="form-group">
                <label>Key State</label>
                <input className="input-field" value={selected?.has_api_key ? "API Key configured" : "API Key not configured"} readOnly />
              </div>
            </div>
          </section>

          <section className="card">
            <div className="card-header">
              <h2 className="card-title">Connection & Model</h2>
              <p className="card-desc">Configure the AI provider, endpoint URL, and API key.</p>
            </div>

            <div className="form-row">
              <div className="form-group">
                <label>AI Provider</label>
                <select className="select-field" value={form.provider} onChange={(event) => updateProvider(event.target.value as AiProvider)}>
                  <option value="openai-compatible">OpenAI Compatible</option>
                  <option value="anthropic-compatible">Anthropic Compatible</option>
                  <option value="custom">Custom</option>
                </select>
              </div>
              <div className="form-group">
                <label>Model</label>
                <input className="input-field" value={form.model} placeholder="gpt-4o-mini" onChange={(event) => updateForm({ ...form, model: event.target.value })} />
              </div>
            </div>

            <div className="form-group" id="endpointGroup">
              <label>API Base URL <span>(Leave empty for default)</span></label>
              <div className="input-wrapper with-left-icon">
                <Link size={16} className="left-input-icon" />
                <input className="input-field" value={form.base_url} placeholder={providerDefaults[form.provider].base_url} onChange={(event) => updateForm({ ...form, base_url: event.target.value })} />
              </div>
            </div>

            <div className="form-row">
              <div className="form-group">
                <label>Request Path</label>
                <input className="input-field" value={form.request_path} placeholder="/chat/completions" onChange={(event) => updateForm({ ...form, request_path: event.target.value })} />
              </div>
              <div className="form-group">
                <label>Auth Header</label>
                <input className="input-field" value={form.auth_header} placeholder="Authorization" onChange={(event) => updateForm({ ...form, auth_header: event.target.value })} />
              </div>
            </div>

            <div className="form-group">
              <label>API Key</label>
              <div className="input-wrapper with-left-icon">
                <Key size={16} className="left-input-icon" />
                <input
                  className="input-field api-key-field"
                  type={showApiKey ? "text" : "password"}
                  value={form.api_key || ""}
                  placeholder={selected?.has_api_key ? "sk-proj-xxxx...xxxx" : "Paste API key"}
                  onChange={(event) => updateForm({ ...form, api_key: event.target.value })}
                />
                <button className="input-icon-btn" type="button" title="Show/Hide" onClick={() => setShowApiKey((value) => !value)}>
                  {showApiKey ? <EyeOff size={16} /> : <Eye size={16} />}
                </button>
              </div>
            </div>
          </section>

          <section className="card">
            <div className="card-header">
              <h2 className="card-title">System Instructions</h2>
              <p className="card-desc">The core prompt that dictates how the AI instance behaves and responds.</p>
            </div>
            <div className="form-group">
              <textarea className="textarea-field" value={form.prompt_template} placeholder="You are an expert..." onChange={(event) => updateForm({ ...form, prompt_template: event.target.value })} />
            </div>
          </section>

          <section className="card">
            <div className="card-header">
              <h2 className="card-title">Advanced Parameters</h2>
              <p className="card-desc">Fine-tune the output characteristics of the model.</p>
            </div>

            <div className="form-row">
              <div className="form-group">
                <label>Temperature</label>
                <div className="slider-container">
                  <input
                    className="range-field"
                    type="range"
                    min="0"
                    max="2"
                    step="0.1"
                    value={form.temperature ?? 0}
                    onChange={(event) => updateForm({ ...form, temperature: Number(event.target.value) })}
                  />
                  <div className="slider-value">{(form.temperature ?? 0).toFixed(1)}</div>
                </div>
                <p className="field-hint">Lower values make output more deterministic.</p>
              </div>
              <div className="form-group">
                <label>Max Tokens</label>
                <input className="input-field" type="number" min="1" value={form.max_tokens ?? ""} onChange={(event) => updateForm({ ...form, max_tokens: event.target.value === "" ? undefined : Number(event.target.value) })} />
              </div>
            </div>

            <div className="form-row">
              <div className="form-group">
                <label>Top P</label>
                <input className="input-field" type="number" step="0.05" value={form.top_p ?? ""} onChange={(event) => updateForm({ ...form, top_p: event.target.value === "" ? undefined : Number(event.target.value) })} />
              </div>
              <div className="form-group">
                <label>Timeout (ms)</label>
                <input className="input-field" type="number" value={form.timeout_ms} onChange={(event) => updateForm({ ...form, timeout_ms: Number(event.target.value) })} />
              </div>
            </div>

            <div className="form-row">
              <div className="form-group">
                <label>Reasoning Effort</label>
                <select className="select-field" value={form.reasoning_effort || ""} onChange={(event) => updateForm({ ...form, reasoning_effort: event.target.value ? event.target.value as AiInstanceFormState["reasoning_effort"] : undefined })}>
                  <option value="">Not Set</option>
                  <option value="low">Low</option>
                  <option value="medium">Medium</option>
                  <option value="high">High</option>
                </select>
              </div>
              <div className="form-group">
                <label>Stream</label>
                <select className="select-field" value={form.stream ? "true" : "false"} onChange={(event) => updateForm({ ...form, stream: event.target.value === "true" })}>
                  <option value="false">Disabled</option>
                  <option value="true">Enabled</option>
                </select>
              </div>
            </div>

            <div className="form-row">
              <div className="form-group">
                <label>Frequency Penalty</label>
                <input className="input-field" type="number" step="0.1" value={form.frequency_penalty ?? ""} onChange={(event) => updateForm({ ...form, frequency_penalty: event.target.value === "" ? undefined : Number(event.target.value) })} />
              </div>
              <div className="form-group">
                <label>Presence Penalty</label>
                <input className="input-field" type="number" step="0.1" value={form.presence_penalty ?? ""} onChange={(event) => updateForm({ ...form, presence_penalty: event.target.value === "" ? undefined : Number(event.target.value) })} />
              </div>
            </div>

            <div className="form-group">
              <label>Stop Sequences</label>
              <textarea className="textarea-field compact-textarea" value={form.stop_text} placeholder="One stop sequence per line, comma is also supported." onChange={(event) => updateForm({ ...form, stop_text: event.target.value })} />
            </div>
          </section>

          <section className="card">
            <div className="card-header">
              <h2 className="card-title">Test Run</h2>
              <p className="card-desc">Send a small test message with this instance and inspect the model reply.</p>
            </div>
            <div className="form-group">
              <label>Test Message</label>
              <textarea className="textarea-field compact-textarea" value={testMessage} onChange={(event) => setTestMessage(event.target.value)} />
            </div>
            {testReply ? (
              <div className="test-reply-box">
                <label>AI Reply</label>
                <pre>{testReply}</pre>
              </div>
            ) : null}
          </section>

          <section className="card danger-card">
            <div className="card-header">
              <h2 className="card-title">Danger Zone</h2>
              <p className="card-desc">Delete only removes this AI instance configuration. The default `.env` model remains unchanged.</p>
            </div>
            <button className="btn-danger" type="button" disabled={!form.id || saving} onClick={deleteInstance}>
              <Trash2 size={16} />
              Delete Agent
            </button>
          </section>

          <div className="agent-bottom-space" />
        </div>
      </main>
    </>
  );
}
