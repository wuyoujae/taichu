import { useEffect, useRef, useState } from "react";
import "./OlympusWorkspace.css";

const workspaceTools = [
  { icon: "presentation", label: "AI Slides" },
  { icon: "table", label: "AI Sheets" },
  { icon: "file-text", label: "AI Docs" },
  { icon: "pen-tool", label: "AI Designer" },
  { icon: "message-circle", label: "AI Chat" },
  { icon: "image", label: "AI Image" },
  { icon: "music", label: "AI Music" },
];

const taskItems = [
  {
    icon: "file-text",
    title: "Olympus Q3 Marketing Strategy Draft",
    date: "Today",
  },
  {
    icon: "edit-3",
    title: "Redesign landing page layout based on Shadcn",
    date: "Mar 24",
  },
  {
    icon: "message-square",
    title: "Brainstorming session for new AI features",
    date: "Mar 22",
  },
  {
    icon: "presentation",
    title: "Create a 20-page slide deck for investors",
    date: "Mar 16",
  },
  {
    icon: "code",
    title: "Generate Python script for data scraping",
    date: "Feb 28",
  },
];

export function OlympusWorkspace() {
  const [collapsed, setCollapsed] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement | null>(null);

  useEffect(() => {
    const api = (window as unknown as { lucide?: { createIcons: () => void } }).lucide;
    if (api?.createIcons) {
      api.createIcons();
    }
  }, [collapsed]);

  useEffect(() => {
    const textarea = textareaRef.current;
    if (!textarea) return;

    const handleInput = () => {
      textarea.style.height = "auto";
      textarea.style.height = `${textarea.scrollHeight}px`;
      textarea.style.overflowY = textarea.scrollHeight > 200 ? "auto" : "hidden";
    };

    textarea.addEventListener("input", handleInput);
    return () => textarea.removeEventListener("input", handleInput);
  }, []);

  return (
    <div className="app-container">
      <nav className="sidebar-primary">
        <div className="logo-box">O</div>
        <button className="nav-item" type="button">
          <i data-lucide="plus" stroke-width="2.5" size="20" />
          <span>New</span>
        </button>
        <button className="nav-item active" type="button">
          <i data-lucide="home" stroke-width="2" size="20" />
          <span>Home</span>
        </button>
        <button className="nav-item" type="button">
          <i data-lucide="network" stroke-width="2" size="20" />
          <span>Agents</span>
        </button>
        <button className="nav-item" type="button">
          <i data-lucide="workflow" stroke-width="2" size="20" />
          <span>Flows</span>
        </button>
        <button className="nav-item" type="button">
          <i data-lucide="folder-open" stroke-width="2" size="20" />
          <span>Drive</span>
        </button>

        <div className="sidebar-bottom">
          <button className="nav-item" type="button">
            <i data-lucide="circle-user-round" stroke-width="2" size="24" />
          </button>
        </div>
      </nav>

      <aside className={`sidebar-secondary${collapsed ? " collapsed" : ""}`} id="taskSidebar">
        <div className="task-header">
          <h2>Task List</h2>
          <div className="task-header-actions">
            <button type="button">
              <i data-lucide="rotate-cw" size="16" />
            </button>
            <button type="button">
              <i data-lucide="filter" size="16" />
            </button>
          </div>
        </div>

        <div className="search-container">
          <div className="search-box">
            <i data-lucide="search" size="16" color="var(--muted-foreground)" />
            <input type="text" placeholder="Search Chats..." />
          </div>
        </div>

        <ul className="task-list">
          {taskItems.map((task) => (
            <li key={`${task.title}-${task.date}`} className="task-item">
              <i data-lucide={task.icon} size="16" className="task-icon" />
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
            <i data-lucide="panel-left" size="20" />
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
            />

            <div className="prompt-toolbar">
              <div className="toolbar-left">
                <button className="btn-icon" type="button" title="Add Attachment">
                  <i data-lucide="plus" size="18" />
                </button>
                <button className="btn-icon" type="button" title="Tools">
                  <i data-lucide="wrench" size="18" />
                </button>
                <button className="btn-model-select" type="button">
                  <i data-lucide="sparkles" size="14" />
                  Standard
                  <i data-lucide="chevron-down" size="14" />
                </button>
              </div>
              <div className="toolbar-right">
                <button className="btn-icon" type="button" title="Voice Input">
                  <i data-lucide="mic" size="18" />
                </button>
                <button className="btn-speak" type="button">
                  <i data-lucide="audio-lines" size="16" />
                  Speak
                </button>
              </div>
            </div>
          </div>

          <div className="tools-row">
            {workspaceTools.map((tool) => (
              <div key={tool.label} className="tool-item">
                <div className="tool-icon-box">
                  <i data-lucide={tool.icon} size="20" />
                </div>
                <span className="tool-label">{tool.label}</span>
              </div>
            ))}
          </div>
        </div>
      </main>
    </div>
  );
}
