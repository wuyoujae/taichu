import { message } from "../components/message";

export type ApiResponse<T> = {
  success: boolean;
  data?: T;
  message?: string;
};

const backendBase = (import.meta.env.VITE_BACKEND_BASE || "http://127.0.0.1:4000").replace(/\/$/, "");

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  let response: Response;

  try {
    response = await fetch(`${backendBase}${path}`, {
      ...init,
      headers: {
        "Content-Type": "application/json",
        ...(init?.headers || {}),
      },
    });
  } catch (error) {
    const errorMessage = error instanceof Error ? error.message : "Network request failed";
    message.error("Network request failed", errorMessage);
    throw new Error(errorMessage);
  }

  const payload = (await response.json().catch(() => null)) as ApiResponse<T> | null;
  if (!response.ok) {
    const errorMessage = payload?.message || `Request failed with status ${response.status}`;
    message.error("Request failed", errorMessage);
    throw new Error(errorMessage);
  }
  if (!payload?.success) {
    const errorMessage = payload?.message || "Request failed";
    message.error("Request failed", errorMessage);
    throw new Error(errorMessage);
  }
  return payload.data as T;
}

export const apiClient = {
  get: <T>(path: string) => request<T>(path),
  post: <T>(path: string, body?: unknown) => request<T>(path, {
    method: "POST",
    body: body === undefined ? undefined : JSON.stringify(body),
  }),
  put: <T>(path: string, body?: unknown) => request<T>(path, {
    method: "PUT",
    body: body === undefined ? undefined : JSON.stringify(body),
  }),
  delete: <T>(path: string) => request<T>(path, { method: "DELETE" }),
};
