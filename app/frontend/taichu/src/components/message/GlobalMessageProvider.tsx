import { useEffect, useMemo, useState } from "react";
import {
  AlertTriangle,
  CheckCircle2,
  Info,
  X,
  XCircle,
} from "lucide-react";
import {
  closeToast,
  getMessageSnapshot,
  MessageSnapshot,
  MessageType,
  resolveModal,
  subscribeMessage,
} from "./message";
import "./GlobalMessage.css";

const toastIconMap = {
  success: CheckCircle2,
  info: Info,
  warn: AlertTriangle,
  error: XCircle,
} satisfies Record<MessageType, typeof CheckCircle2>;

export function GlobalMessageProvider() {
  const [snapshot, setSnapshot] = useState<MessageSnapshot>(() => getMessageSnapshot());
  const [promptValue, setPromptValue] = useState("");

  useEffect(() => subscribeMessage(() => setSnapshot(getMessageSnapshot())), []);

  useEffect(() => {
    if (snapshot.modal?.type === "prompt") {
      setPromptValue(snapshot.modal.defaultValue || "");
    }
  }, [snapshot.modal]);

  const modalConfirmClass = useMemo(() => {
    if (!snapshot.modal) return "o-message-btn-primary";
    return snapshot.modal.destructive ? "o-message-btn-danger" : "o-message-btn-primary";
  }, [snapshot.modal]);

  return (
    <>
      <div className="o-toast-container" aria-live="polite" aria-atomic="true">
        {snapshot.toasts.map((toast) => {
          const Icon = toastIconMap[toast.type];

          return (
            <div key={toast.id} className={`o-toast ${toast.closing ? "hide" : "show"}`} role="status">
              <Icon size={20} className={`o-toast-icon ${toast.type}`} />
              <div className="o-toast-content">
                <div className="o-toast-title">{toast.title}</div>
                {toast.desc ? <div className="o-toast-desc">{toast.desc}</div> : null}
              </div>
              <button className="o-toast-close" type="button" aria-label="关闭消息" onClick={() => closeToast(toast.id)}>
                <X size={16} />
              </button>
            </div>
          );
        })}
      </div>

      {snapshot.modal ? (
        <div className="o-modal-overlay show" role="dialog" aria-modal="true" aria-labelledby="o-modal-title">
          <div className="o-modal-box">
            <div className="o-modal-header">
              <h3 className="o-modal-title" id="o-modal-title">{snapshot.modal.title}</h3>
              {snapshot.modal.desc ? <p className="o-modal-desc">{snapshot.modal.desc}</p> : null}
            </div>

            {snapshot.modal.type === "prompt" ? (
              <div className="o-modal-body">
                <input
                  className="o-modal-input"
                  type="text"
                  value={promptValue}
                  placeholder={snapshot.modal.placeholder || ""}
                  autoFocus
                  onChange={(event) => setPromptValue(event.target.value)}
                  onKeyDown={(event) => {
                    if (event.key === "Enter") {
                      resolveModal(promptValue);
                    }
                  }}
                />
              </div>
            ) : null}

            <div className="o-modal-footer">
              <button
                className="o-message-btn o-message-btn-default"
                type="button"
                onClick={() => resolveModal(snapshot.modal?.type === "prompt" ? null : false)}
              >
                {snapshot.modal.cancelText || "Cancel"}
              </button>
              <button
                className={`o-message-btn ${modalConfirmClass}`}
                type="button"
                onClick={() => resolveModal(snapshot.modal?.type === "prompt" ? promptValue : true)}
              >
                {snapshot.modal.confirmText || "Confirm"}
              </button>
            </div>
          </div>
        </div>
      ) : null}
    </>
  );
}
