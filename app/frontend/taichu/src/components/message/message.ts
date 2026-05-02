export type MessageType = "success" | "info" | "warn" | "error";

export type MessageOptions = {
  title: string;
  desc?: string;
  duration?: number;
};

export type ConfirmOptions = {
  title: string;
  desc?: string;
  confirmText?: string;
  cancelText?: string;
  destructive?: boolean;
};

export type PromptOptions = ConfirmOptions & {
  placeholder?: string;
  defaultValue?: string;
};

export type ToastState = MessageOptions & {
  id: number;
  type: MessageType;
  closing: boolean;
};

export type ModalState = (ConfirmOptions | PromptOptions) & {
  id: number;
  type: "confirm" | "prompt";
  resolve: (value: boolean | string | null) => void;
};

export type MessageSnapshot = {
  toasts: ToastState[];
  modal: ModalState | null;
};

type Listener = () => void;

let nextId = 1;
let toasts: ToastState[] = [];
let activeModal: ModalState | null = null;
const listeners = new Set<Listener>();

function emit() {
  listeners.forEach((listener) => listener());
}

function dismissToast(id: number) {
  toasts = toasts.map((toast) => (toast.id === id ? { ...toast, closing: true } : toast));
  emit();

  window.setTimeout(() => {
    toasts = toasts.filter((toast) => toast.id !== id);
    emit();
  }, 300);
}

function createToast(type: MessageType, options: MessageOptions | string) {
  const normalized = typeof options === "string" ? { title: options } : options;
  const toast: ToastState = {
    id: nextId,
    type,
    closing: false,
    duration: 4000,
    ...normalized,
  };

  nextId += 1;
  toasts = [...toasts, toast];
  emit();

  if (toast.duration && toast.duration > 0) {
    window.setTimeout(() => dismissToast(toast.id), toast.duration);
  }

  return toast.id;
}

function createModal(options: ConfirmOptions | PromptOptions, type: ModalState["type"]) {
  return new Promise<boolean | string | null>((resolve) => {
    activeModal = {
      ...options,
      id: nextId,
      type,
      resolve,
    };
    nextId += 1;
    emit();
  });
}

export function subscribeMessage(listener: Listener) {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

export function getMessageSnapshot(): MessageSnapshot {
  return {
    toasts,
    modal: activeModal,
  };
}

export function closeToast(id: number) {
  dismissToast(id);
}

export function resolveModal(value: boolean | string | null) {
  if (!activeModal) return;

  const modal = activeModal;
  activeModal = null;
  emit();
  window.setTimeout(() => modal.resolve(value), 200);
}

export const message = {
  success: (title: string, desc?: string, duration?: number) => createToast("success", { title, desc, duration }),
  info: (title: string, desc?: string, duration?: number) => createToast("info", { title, desc, duration }),
  warn: (title: string, desc?: string, duration?: number) => createToast("warn", { title, desc, duration }),
  warning: (title: string, desc?: string, duration?: number) => createToast("warn", { title, desc, duration }),
  error: (title: string, desc?: string, duration?: number) => createToast("error", { title, desc, duration }),
  open: createToast,
  close: closeToast,
  confirm: (options: ConfirmOptions) => createModal(options, "confirm") as Promise<boolean>,
  prompt: (options: PromptOptions) => createModal(options, "prompt") as Promise<string | null>,
};
