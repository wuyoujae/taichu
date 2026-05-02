import { useRef, useState } from "react";
import {
  ChevronDown,
  CircleUserRound,
  Code,
  Edit3,
  FileText,
  FolderOpen,
  Image as ImageIcon,
  Mic,
  MessageCircle,
  MessageSquare,
  Music,
  PenTool,
  Plus,
  Presentation,
  PanelLeft,
  Search,
  Network,
  Filter,
  Sparkles,
  Table,
  Wrench,
  AudioLines,
  BookOpen,
  Server,
  Workflow,
} from "lucide-react";
import { AgentAiSettingsPage } from "./AgentAiSettingsPage";
import { McpSettingsPage } from "./McpSettingsPage";
import { SkillsSettingsPage } from "./SkillsSettingsPage";
import "./OlympusWorkspace.css";

type WorkspaceTab = "home" | "ai" | "skills" | "mcp";

const taskItems = [
  { Icon: FileText, title: "Olympus Q3 Marketing Strategy Draft", date: "Today" },
  { Icon: Edit3, title: "Redesign landing page layout based on Shadcn", date: "Mar 24" },
  { Icon: MessageSquare, title: "Brainstorming session for new AI features", date: "Mar 22" },
  { Icon: Presentation, title: "Create a 20-page slide deck for investors", date: "Mar 16" },
  { Icon: Code, title: "Generate Python script for data scraping", date: "Feb 28" },
];

const workspaceTools = [
  { Icon: Presentation, label: "AI Slides" },
  { Icon: Table, label: "AI Sheets" },
  { Icon: FileText, label: "AI Docs" },
  { Icon: PenTool, label: "AI Designer" },
  { Icon: MessageCircle, label: "AI Chat" },
  { Icon: ImageIcon, label: "AI Image" },
  { Icon: Music, label: "AI Music" },
];

export function OlympusWorkspace() {
  const [collapsed, setCollapsed] = useState(false);
  const [activeTab, setActiveTab] = useState<WorkspaceTab>("home");
  const textareaRef = useRef<HTMLTextAreaElement | null>(null);

  const adjustHeight = () => {
    const textarea = textareaRef.current;
    if (!textarea) return;

    textarea.style.height = "auto";
    textarea.style.height = `${textarea.scrollHeight}px`;
    textarea.style.overflowY = textarea.scrollHeight > 200 ? "auto" : "hidden";
  };

  return (
    <div className="app-container">
      <nav className="sidebar-primary">
        <div className="logo-box">O</div>
        <button className="nav-item" type="button">
          <Plus size={20} strokeWidth={2.5} />
          <span>New</span>
        </button>
        <button
          className={`nav-item${activeTab === "home" ? " active" : ""}`}
          type="button"
          onClick={() => setActiveTab("home")}
        >
          <span className="nav-icon-holder">
            <Workflow size={20} strokeWidth={2} />
          </span>
          <span>Home</span>
        </button>
        <button
          className={`nav-item${activeTab === "ai" ? " active" : ""}`}
          type="button"
          onClick={() => setActiveTab("ai")}
        >
          <span className="nav-icon-holder">
            <Network size={20} strokeWidth={2} />
          </span>
          <span>AI</span>
        </button>
        <button
          className={`nav-item${activeTab === "skills" ? " active" : ""}`}
          type="button"
          onClick={() => setActiveTab("skills")}
        >
          <span className="nav-icon-holder">
            <BookOpen size={20} strokeWidth={2} />
          </span>
          <span>Skills</span>
        </button>
        <button
          className={`nav-item${activeTab === "mcp" ? " active" : ""}`}
          type="button"
          onClick={() => setActiveTab("mcp")}
        >
          <span className="nav-icon-holder">
            <Server size={20} strokeWidth={2} />
          </span>
          <span>MCP</span>
        </button>
        <button className="nav-item" type="button">
          <span className="nav-icon-holder">
            <Workflow size={20} strokeWidth={2} />
          </span>
          <span>Flows</span>
        </button>
        <button className="nav-item" type="button">
          <span className="nav-icon-holder">
            <FolderOpen size={20} strokeWidth={2} />
          </span>
          <span>Drive</span>
        </button>

        <div className="sidebar-bottom">
          <button className="nav-item" type="button">
            <CircleUserRound size={24} strokeWidth={2} />
          </button>
        </div>
      </nav>

      {activeTab === "ai" ? (
        <AgentAiSettingsPage />
      ) : activeTab === "skills" ? (
        <SkillsSettingsPage />
      ) : activeTab === "mcp" ? (
        <McpSettingsPage />
      ) : (
        <>
      <aside className={`sidebar-secondary${collapsed ? " collapsed" : ""}`} id="taskSidebar">
        <div className="task-header">
          <h2>Task List</h2>
        <div className="task-header-actions">
          <button type="button">
              <Workflow size={16} strokeWidth={2} />
          </button>
          <button type="button">
              <Filter size={16} />
          </button>
        </div>
        </div>

        <div className="search-container">
          <div className="search-box">
            <Search size={16} />
            <input type="text" placeholder="Search Chats..." />
          </div>
        </div>

        <ul className="task-list">
          {taskItems.map((task) => (
            <li key={`${task.title}-${task.date}`} className="task-item">
              <task.Icon size={16} className="task-icon" />
              <div className="task-content">
                <div className="task-title">{task.title}</div>
                <div className="task-date">{task.date}</div>
              </div>
            </li>
          ))}
        </ul>
      </aside>

      <main className="main-content">
        <header className="top-nav">
          <button
            className="toggle-btn"
            id="toggleSidebarBtn"
            type="button"
            onClick={() => setCollapsed((value) => !value)}
          >
            <PanelLeft size={20} />
          </button>

          <div className="top-actions">
            <button className="btn-upgrade" type="button">
            Upgrade Pro
            </button>
          </div>
        </header>

          <div className="workspace-center">
            <h1 className="workspace-title">Olympus AI Workspace</h1>

            <div className="prompt-wrapper">
              <textarea
                ref={textareaRef}
                className="prompt-input"
                placeholder="Ask anything, create anything..."
                rows={1}
                onInput={adjustHeight}
              />

              <div className="prompt-toolbar">
                <div className="toolbar-left">
                  <button className="btn-icon" type="button" title="Add Attachment">
                    <Plus size={18} />
                  </button>
                  <button className="btn-icon" type="button" title="Tools">
                    <Wrench size={18} />
                  </button>
                  <button className="btn-model-select" type="button">
                    <Sparkles size={14} />
                    Standard
                    <ChevronDown size={14} />
                  </button>
                </div>
                <div className="toolbar-right">
                  <button className="btn-icon" type="button" title="Voice Input">
                    <Mic size={18} />
                  </button>
                  <button className="btn-speak" type="button">
                    <AudioLines size={16} />
                    Speak
                  </button>
                </div>
              </div>
            </div>

            <div className="tools-row">
              {workspaceTools.map((tool) => (
                <div key={tool.label} className="tool-item">
                  <div className="tool-icon-box">
                    <tool.Icon size={20} />
                  </div>
                  <span className="tool-label">{tool.label}</span>
                </div>
              ))}
            </div>
          </div>
      </main>
        </>
      )}
    </div>
  );
}
