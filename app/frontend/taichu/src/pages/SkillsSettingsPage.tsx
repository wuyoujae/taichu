import { useEffect, useMemo, useState } from "react";
import { Blocks, Check, Download, FileText, Loader2, Plus, Search, Settings2, Trash2, Users, X } from "lucide-react";
import { apiClient } from "../api/client";
import { message } from "../components/message";
import type { InstalledSkill, SkillDescriptor, SkillLoadResult, SkillsConfigView, SkillStatusCode } from "../types/configPages";
import "./SkillsSettingsPage.css";

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
  const [sheetOpen, setSheetOpen] = useState(false);
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

  async function openSheet(skill: SkillDescriptor) {
    setSelected(skill);
    setLoadedSkill(null);
    setSheetOpen(true);
    if (skill.enabled) {
      try {
        setLoadedSkill(await apiClient.get<SkillLoadResult>(`/yuanling/skills/${encodeURIComponent(skill.id)}`));
      } catch {
        setLoadedSkill(null);
      }
    }
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

  async function triggerImport() {
    const sourcePath = await message.prompt({
      title: "Import Skill",
      desc: "Enter a local SKILL.md path or a directory containing SKILL.md.",
      placeholder: "/Users/you/skills/writer/SKILL.md",
      confirmText: "Import",
    });
    if (!sourcePath) return;
    const installed = await apiClient.post<InstalledSkill>("/yuanling/skills/install", { source_path: sourcePath });
    message.success("Skill Imported", `${installed.name} has been installed.`);
    await loadSkills(installed.id);
  }

  return (
    <main className="main-content skills-prototype">
      <header className="page-header">
        <div className="header-title">
          <h1>Skills Library</h1>
          <p>Manage local SKILL.md instructions your Yuanling agents can use.</p>
        </div>
        <div className="header-actions">
          <button className="btn btn-default" type="button" onClick={() => void triggerImport()}><Download size={16} /> Import Skill</button>
          <button className="btn btn-primary" type="button" onClick={() => message.info("Create Skill", "Skill creation editor will use the same SKILL.md format.")}><Plus size={16} /> Create Skill</button>
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
              <button className="agent-count" type="button" onClick={() => void openSheet(skill)} title="Manage Skill">
                <Users size={14} /> {config?.auto_inject_enabled ? "Auto Inject" : "Manual"}
              </button>
              <button
                type="button"
                className="switch"
                role="switch"
                aria-checked={skill.enabled}
                disabled={savingId === skill.id}
                onClick={() => void updateStatus(skill, skill.enabled ? 2 : 1)}
              >
                <span className="switch-thumb" />
              </button>
              <div className="item-divider" />
              <button className="btn-icon" type="button" onClick={() => void openSheet(skill)}><Settings2 size={16} /></button>
              <button className="btn-icon danger" type="button" onClick={() => void confirmDelete(skill)}><Trash2 size={16} /></button>
            </div>
          </div>
        )) : <div className="empty-row">No skills found.</div>}
      </div>

      <div className={`overlay${sheetOpen ? " show" : ""}`} onClick={() => setSheetOpen(false)} />
      <div className={`sheet${sheetOpen ? " open" : ""}`}>
        <div className="sheet-header">
          <h2 className="sheet-title">{selected ? `${selected.name} Permissions` : "Manage Skill"}</h2>
          <button className="sheet-close" type="button" onClick={() => setSheetOpen(false)}><X size={20} /></button>
        </div>
        <div className="sheet-body">
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

          <div className="form-section">
            <h3 className="section-title">Skill Details</h3>
            <div className="detail-lines">
              <p><strong>ID</strong><span>{selected?.id || "-"}</span></p>
              <p><strong>Source</strong><span>{selected?.source || "-"}</span></p>
              <p><strong>Path</strong><span>{selected?.path || "-"}</span></p>
            </div>
          </div>

          <div className="form-section">
            <h3 className="section-title">Prompt Preview</h3>
            <pre className="prompt-preview"><FileText size={14} /> {loadedSkill?.prompt || "Only active skills can load their full SKILL.md prompt."}</pre>
          </div>
        </div>
        <div className="sheet-footer">
          <button className="btn btn-default" type="button" onClick={() => setSheetOpen(false)}>Cancel</button>
          <button className="btn btn-primary" type="button" onClick={() => { setSheetOpen(false); message.success("Permissions Updated", "Skill configuration has been saved."); }}>Save Changes</button>
        </div>
      </div>
    </main>
  );
}
