import type { Dispatch, SetStateAction } from "react";

export type ToastType = "success" | "error" | "warning" | "info";

export interface ToastItem {
  id: string;
  type: ToastType;
  message: string;
  duration?: number;
}

let toastIdCounter = 0;
let globalSetToasts: Dispatch<SetStateAction<ToastItem[]>> | null = null;

export function bindToastStore(setToasts: Dispatch<SetStateAction<ToastItem[]>>) {
  globalSetToasts = setToasts;
  return () => {
    if (globalSetToasts === setToasts) {
      globalSetToasts = null;
    }
  };
}

export function showToast(
  type: ToastType,
  message: string,
  duration?: number
) {
  const id = `toast_${++toastIdCounter}_${Date.now()}`;
  const newToast: ToastItem = { id, type, message, duration };

  if (globalSetToasts) {
    globalSetToasts((prev) => [...prev, newToast]);
  }
}
