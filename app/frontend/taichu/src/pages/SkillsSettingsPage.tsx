import { useEffect, useMemo, useState } from "react";
import { Blocks, Check, Download, FileText, Loader2, Plus, Search, Settings2, Trash2, Users, X } from "lucide-react";
import { apiClient } from "../api/client";
import { message } from "../components/message";
import type { InstalledSkill, SkillDescriptor, SkillLoadResult, SkillsConfigView, SkillStatusCode } from "../types/configPages";
import "./SkillsSettingsPage.css";

type SkillSheetMode = "manage" | "import" | "create";

function sourceBadge(skill: SkillDescriptor) {
  if (skill.source === "data_dir") return "Official";
  if (skill.source === "runtime") return "Runtime";
  if (skill.source === "user_home") return "User";
  return "Custom";
}

export function SkillsSettingsPage() {
  const [config, setConfig] = useState<SkillsConfigView | null>(null);
  const [skills, setSkills] = useState<SkillDescriptor[]>([]);
  const [selected, setSelected] = useState<SkillDescriptor | null>(null);
  const [loadedSkill, setLoadedSkill] = useState<SkillLoadResult | null>(null);
  const [editForm, setEditForm] = useState({ name: "", description: "", prompt: "" });
  const [sheetOpen, setSheetOpen] = useState(false);
  const [sheetMode, setSheetMode] = useState<SkillSheetMode>("manage");
  const [importPath, setImportPath] = useState("");
  const [createForm, setCreateForm] = useState({
    name: "",
    description: "",
    prompt: "# New Skill\n\nDescribe when and how Yuanling should use this skill.",
  });
  const [loading, setLoading] = useState(true);
  const [savingId, setSavingId] = useState<string | null>(null);
  const [search, setSearch] = useState("");

  async function loadSkills(nextSelectedId?: string) {
    setLoading(true);
    try {
      const data = await apiClient.get<SkillsConfigView>("/yuanling/skills/config");
      setConfig(data);
      setSkills(data.skills);
      const target = nextSelectedId ? data.skills.find((item) => item.id === nextSelectedId) : data.skills[0];
      if (target) setSelected(target);
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void loadSkills();
  }, []);

  const filteredSkills = useMemo(() => {
    const query = search.trim().toLowerCase();
    if (!query) return skills;
    return skills.filter((skill) => [skill.id, skill.name, skill.description || "", skill.source].join(" ").toLowerCase().includes(query));
  }, [search, skills]);

  async function openManageSheet(skill: SkillDescriptor) {
    setSheetMode("manage");
    setSelected(skill);
    setLoadedSkill(null);
    setSheetOpen(true);
    if (skill.enabled) {
      try {
        const detail = await apiClient.get<SkillLoadResult>(`/yuanling/skills/${encodeURIComponent(skill.id)}`);
        setLoadedSkill(detail);
        setEditForm({ name: detail.name, description: detail.description || "", prompt: detail.prompt });
      } catch {
        setLoadedSkill(null);
        setEditForm({ name: skill.name, description: skill.description || "", prompt: "" });
      }
    } else {
      setEditForm({ name: skill.name, description: skill.description || "", prompt: "" });
    }
  }

  function openImportSheet() {
    setSheetMode("import");
    setImportPath("");
    setSheetOpen(true);
  }

  function openCreateSheet() {
    setSheetMode("create");
    setCreateForm({
      name: "",
      description: "",
      prompt: "# New Skill\n\nDescribe when and how Yuanling should use this skill.",
    });
    setSheetOpen(true);
  }

  async function updateStatus(skill: SkillDescriptor, status: SkillStatusCode) {
    setSavingId(skill.id);
    try {
      const updated = await apiClient.put<SkillDescriptor>(`/yuanling/skills/${encodeURIComponent(skill.id)}/status`, { status });
      message.success("Skill Updated", `${updated.name} status has been saved.`);
      await loadSkills(updated.id);
    } finally {
      setSavingId(null);
    }
  }

  async function confirmDelete(skill: SkillDescriptor) {
    const ok = await message.confirm({
      title: `Delete "${skill.name}"?`,
      desc: "This marks the skill as deleted. The local SKILL.md file will not be physically removed.",
      confirmText: "Delete",
      destructive: true,
    });
    if (ok) await updateStatus(skill, 3);
  }

  async function submitImport() {
    if (!importPath.trim()) {
      message.warning("Import path required", "Please enter a SKILL.md path or a folder containing SKILL.md.");
      return;
    }
    const installed = await apiClient.post<InstalledSkill>("/yuanling/skills/install", { source_path: importPath.trim() });
    message.success("Skill Imported", `${installed.name} has been installed.`);
    setSheetOpen(false);
    await loadSkills(installed.id);
  }

  async function submitCreate() {
    if (!createForm.name.trim() || !createForm.prompt.trim()) {
      message.warning("Skill info required", "Name and prompt are required to create a skill.");
      return;
    }
    const installed = await apiClient.post<InstalledSkill>("/yuanling/skills/create", createForm);
    message.success("Skill Created", `${installed.name} has been created.`);
    setSheetOpen(false);
    await loadSkills(installed.id);
  }

  function sheetTitle() {
    if (sheetMode === "import") return "Import Skill";
    if (sheetMode === "create") return "Create Skill";
    return selected ? `${selected.name} Permissions` : "Manage Skill";
  }

  async function submitUpdate() {
    if (!selected) return;
    const updated = await apiClient.put<SkillDescriptor>(`/yuanling/skills/${encodeURIComponent(selected.id)}`, editForm);
    message.success("Skill Saved", `${updated.name} has been updated.`);
    setSheetOpen(false);
    await loadSkills(updated.id);
  }

  function submitSheet() {
    if (sheetMode === "import") void submitImport();
    else if (sheetMode === "create") void submitCreate();
    else void submitUpdate();
  }

  return (
    <main className="main-content skills-prototype">
      <header className="page-header">
        <div className="header-title">
          <h1>Skills Library</h1>
          <p>Manage local SKILL.md instructions your Yuanling agents can use.</p>
        </div>
        <div className="header-actions">
          <button className="btn btn-default" type="button" onClick={openImportSheet}><Download size={16} /> Import Skill</button>
          <button className="btn btn-primary" type="button" onClick={openCreateSheet}><Plus size={16} /> Create Skill</button>
        </div>
      </header>

      <div className="toolbar">
        <div className="search-input-box">
          <Search size={16} />
          <input className="search-input" value={search} onChange={(event) => setSearch(event.target.value)} placeholder="Search skills..." />
        </div>
      </div>

      <div className="list-container">
        {loading ? (
          <div className="empty-row"><Loader2 className="spin" size={18} /> Loading skills...</div>
        ) : filteredSkills.length ? filteredSkills.map((skill) => (
          <div className="list-item" key={skill.id}>
            <div className="item-icon"><Blocks size={20} /></div>
            <div className="item-info">
              <div className="item-title-row">
                <span className="item-title">{skill.name}</span>
                <span className={`badge${skill.enabled ? " active" : ""}`}>{sourceBadge(skill)}</span>
                {!skill.enabled ? <span className="badge danger">{skill.status_label}</span> : null}
              </div>
              <div className="item-desc">{skill.description || "No description provided."}</div>
            </div>
            <div className="item-actions">
              <button className="agent-count" type="button" onClick={() => void openManageSheet(skill)} title="Manage Skill">
                <Users size={14} /> {config?.auto_inject_enabled ? "Auto Inject" : "Manual"}
              </button>
              <button type="button" className="switch" role="switch" aria-checked={skill.enabled} disabled={savingId === skill.id} onClick={() => void updateStatus(skill, skill.enabled ? 2 : 1)}>
                <span className="switch-thumb" />
              </button>
              <div className="item-divider" />
              <button className="btn-icon" type="button" onClick={() => void openManageSheet(skill)}><Settings2 size={16} /></button>
              <button className="btn-icon danger" type="button" onClick={() => void confirmDelete(skill)}><Trash2 size={16} /></button>
            </div>
          </div>
        )) : <div className="empty-row">No skills found.</div>}
      </div>

      <div className={`overlay${sheetOpen ? " show" : ""}`} onClick={() => setSheetOpen(false)} />
      <div className={`sheet${sheetOpen ? " open" : ""}`}>
        <div className="sheet-header">
          <h2 className="sheet-title">{sheetTitle()}</h2>
          <button className="sheet-close" type="button" onClick={() => setSheetOpen(false)}><X size={20} /></button>
        </div>
        <div className="sheet-body">
          {sheetMode === "import" ? (
            <div className="form-section">
              <h3 className="section-title">Local SKILL.md Source</h3>
              <p className="section-desc">Enter a local SKILL.md file path or a directory containing SKILL.md.</p>
              <input className="input-field" value={importPath} onChange={(event) => setImportPath(event.target.value)} placeholder="/Users/you/skills/writer/SKILL.md" autoFocus />
            </div>
          ) : sheetMode === "create" ? (
            <>
              <div className="form-section create-fields">
                <h3 className="section-title">Skill Metadata</h3>
                <input className="input-field" value={createForm.name} onChange={(event) => setCreateForm({ ...createForm, name: event.target.value })} placeholder="Skill name" autoFocus />
                <input className="input-field" value={createForm.description} onChange={(event) => setCreateForm({ ...createForm, description: event.target.value })} placeholder="Short description" />
              </div>
              <div className="form-section">
                <h3 className="section-title">Instructions</h3>
                <textarea className="textarea-field" value={createForm.prompt} onChange={(event) => setCreateForm({ ...createForm, prompt: event.target.value })} />
              </div>
            </>
          ) : (
            <>
              <div className="form-section">
                <h3 className="section-title">Skill Status</h3>
                <div className="checkbox-list">
                  {[{ code: 1 as SkillStatusCode, title: "Active", desc: "Visible to context injection and Skill tool." }, { code: 2 as SkillStatusCode, title: "Disabled", desc: "Kept on disk but hidden from active use." }, { code: 3 as SkillStatusCode, title: "Deleted", desc: "Soft delete state without removing the file." }].map((item) => (
                    <button key={item.code} className="checkbox-item" data-selected={selected?.status === item.code} type="button" onClick={() => selected && void updateStatus(selected, item.code)}>
                      <div className="custom-checkbox"><Check size={12} className="check-icon" strokeWidth={3} /></div>
                      <div className="checkbox-info"><h4>{item.title}</h4><p>{item.desc}</p></div>
                    </button>
                  ))}
                </div>
              </div>
              <div className="form-section create-fields">
                <h3 className="section-title">Skill Details</h3>
                <input className="input-field" value={editForm.name} onChange={(event) => setEditForm({ ...editForm, name: event.target.value })} placeholder="Skill name" />
                <input className="input-field" value={editForm.description} onChange={(event) => setEditForm({ ...editForm, description: event.target.value })} placeholder="Short description" />
                <div className="detail-lines">
                  <p><strong>ID</strong><span>{selected?.id || "-"}</span></p>
                  <p><strong>Source</strong><span>{selected?.source || "-"}</span></p>
                  <p><strong>Path</strong><span>{selected?.path || "-"}</span></p>
                </div>
              </div>
              <div className="form-section">
                <h3 className="section-title">Instructions</h3>
                <textarea className="textarea-field" value={editForm.prompt} onChange={(event) => setEditForm({ ...editForm, prompt: event.target.value })} placeholder={loadedSkill ? "Skill instructions" : "Enable this skill to load and edit full instructions."} />
              </div>
            </>
          )}
        </div>
        <div className="sheet-footer">
          <button className="btn btn-default" type="button" onClick={() => setSheetOpen(false)}>Cancel</button>
          <button className="btn btn-primary" type="button" onClick={submitSheet}>{sheetMode === "import" ? "Import Skill" : sheetMode === "create" ? "Create Skill" : "Save Changes"}</button>
        </div>
      </div>
    </main>
  );
}
