import { onBeforeUnmount, reactive, watchEffect } from "vue";

/** Action affichée dans la barre du bas, avec son raccourci clavier. */
export interface ShellAction {
  /** Raccourci : "Enter", "Delete", "x", "Ctrl+e"… (lettres en minuscules). */
  key: string;
  /** Libellé de la touche affiché dans la pastille (par défaut : `key`). */
  cap?: string;
  label: string;
  run: () => void;
  disabled?: boolean;
}

export const shell = reactive({
  hint: "",
  actions: [] as ShellAction[],
});

/** Déclare l'aide et les actions de la page affichée (mises à jour automatiquement). */
export function useShell(build: () => { hint?: string; actions?: ShellAction[] }) {
  const stop = watchEffect(() => {
    const b = build();
    shell.hint = b.hint ?? "";
    shell.actions = b.actions ?? [];
  });
  onBeforeUnmount(() => {
    stop();
    shell.hint = "";
    shell.actions = [];
  });
}

/** Représentation d'un événement clavier au format des raccourcis (« Ctrl+e », « Delete »…). */
export function keyOf(e: KeyboardEvent) {
  const k = e.key.length === 1 ? e.key.toLowerCase() : e.key;
  return `${e.ctrlKey || e.metaKey ? "Ctrl+" : ""}${e.shiftKey && e.key.length > 1 ? "Shift+" : ""}${k}`;
}

/** Vrai si le focus est dans un champ de saisie (les raccourcis à une lettre sont alors ignorés). */
export function typing(e: KeyboardEvent) {
  const t = e.target as HTMLElement | null;
  return !!t && (t.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(t.tagName));
}
