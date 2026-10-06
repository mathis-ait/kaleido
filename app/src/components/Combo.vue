<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import type { MoveCat, TypeTag } from "../types";
import CategoryIcon from "./CategoryIcon.vue";
import Sprite from "./Sprite.vue";
import TypeBadge from "./TypeBadge.vue";

/**
 * Liste déroulante avec recherche (espèces, attaques, objets, lieux…).
 * Tape pour filtrer (accents ignorés, numéro accepté), flèches + Entrée pour choisir.
 */
export interface ComboOption {
  value: number;
  label: string;
  /** Texte secondaire à droite (numéro, type…). */
  hint?: string;
  /** Espèce dont l'icône est affichée. */
  sprite?: number;
  /** Attaques : type et catégorie affichés à droite. */
  type?: TypeTag;
  category?: MoveCat;
  /** Choix conseillé (ex. attaque apprenable), surligné en vert. */
  good?: boolean;
}

const props = withDefaults(
  defineProps<{ options: ComboOption[]; placeholder?: string; disabled?: boolean; sprites?: boolean; noneLabel?: string }>(),
  { placeholder: "Rechercher…", disabled: false, sprites: false, noneLabel: undefined },
);
const model = defineModel<number>({ required: true });

const MAX_SHOWN = 120;
const fold = (s: string) => s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();

const open = ref(false);
const query = ref("");
const active = ref(0);
const input = ref<HTMLInputElement | null>(null);
const list = ref<HTMLElement | null>(null);
const pos = ref({ left: 0, top: 0, width: 0, above: false });

const all = computed<ComboOption[]>(() =>
  props.noneLabel !== undefined ? [{ value: 0, label: props.noneLabel }, ...props.options.filter((o) => o.value !== 0)] : props.options,
);
const current = computed(() => all.value.find((o) => o.value === model.value));
const filtered = computed(() => {
  const q = fold(query.value.trim());
  if (!q) return all.value.slice(0, MAX_SHOWN);
  const num = /^\d+$/.test(q) ? Number(q) : null;
  const starts: ComboOption[] = [];
  const contains: ComboOption[] = [];
  for (const o of all.value) {
    if (num !== null && o.value === num) starts.unshift(o);
    const l = fold(o.label);
    if (l.startsWith(q)) starts.push(o);
    else if (l.includes(q)) contains.push(o);
    if (starts.length > MAX_SHOWN) break;
  }
  return [...starts, ...contains].slice(0, MAX_SHOWN);
});

watch(filtered, () => (active.value = 0));

function place() {
  const r = input.value?.getBoundingClientRect();
  if (!r) return;
  const above = r.bottom + 300 > window.innerHeight && r.top > 300;
  pos.value = { left: r.left, top: above ? r.top - 4 : r.bottom + 4, width: Math.max(r.width, 240), above };
}

function openList() {
  if (props.disabled || open.value) return;
  open.value = true;
  query.value = "";
  place();
  const i = filtered.value.findIndex((o) => o.value === model.value);
  active.value = Math.max(0, i);
  nextTick(scrollActive);
  window.addEventListener("resize", close);
  document.addEventListener("pointerdown", outside, true);
}

function close() {
  open.value = false;
  query.value = "";
  window.removeEventListener("resize", close);
  document.removeEventListener("pointerdown", outside, true);
}

function outside(e: PointerEvent) {
  const t = e.target as Node;
  if (!input.value?.contains(t) && !list.value?.contains(t)) close();
}

function choose(o: ComboOption | undefined) {
  if (o) model.value = o.value;
  close();
  input.value?.blur();
}

function scrollActive() {
  list.value?.querySelector<HTMLElement>(`[data-i="${active.value}"]`)?.scrollIntoView({ block: "nearest" });
}

function onKey(e: KeyboardEvent) {
  if (!open.value && ["ArrowDown", "Enter", " "].includes(e.key)) {
    e.preventDefault();
    openList();
    return;
  }
  if (e.key === "ArrowDown" || e.key === "ArrowUp") {
    e.preventDefault();
    const n = filtered.value.length;
    if (n) active.value = (active.value + (e.key === "ArrowDown" ? 1 : n - 1)) % n;
    nextTick(scrollActive);
  } else if (e.key === "Enter") {
    e.preventDefault();
    choose(filtered.value[active.value]);
  } else if (e.key === "Escape" && open.value) {
    e.stopPropagation();
    close();
  } else if (e.key === "Tab") {
    close();
  }
}

onBeforeUnmount(close);
</script>

<template>
  <div class="combo" :class="{ open, disabled }">
    <Sprite v-if="sprites && current && !open && current.value" :id="current.sprite ?? current.value" :size="28" class="lead" />
    <input
      ref="input"
      class="field"
      :class="{ 'has-lead': sprites && current && !open && current.value }"
      :value="open ? query : (current?.label ?? (model ? `n°${model}` : ''))"
      :placeholder="open ? (current?.label ?? placeholder) : placeholder"
      :disabled="disabled"
      role="combobox"
      :aria-expanded="open"
      autocomplete="off"
      spellcheck="false"
      @focus="openList"
      @click="openList"
      @input="query = ($event.target as HTMLInputElement).value"
      @keydown="onKey"
    />
    <svg class="chevron" viewBox="0 0 24 24" aria-hidden="true"><path d="m6 9 6 6 6-6" /></svg>
    <Teleport to="body">
      <ul
        v-if="open"
        ref="list"
        class="combo-list"
        role="listbox"
        :class="{ above: pos.above }"
        :style="{ left: `${pos.left}px`, top: `${pos.top}px`, width: `${pos.width}px` }"
      >
        <li
          v-for="(o, i) in filtered"
          :key="o.value"
          :data-i="i"
          role="option"
          :aria-selected="o.value === model"
          :class="{ active: i === active, chosen: o.value === model, good: o.good }"
          @pointerenter="active = i"
          @pointerdown.prevent="choose(o)"
        >
          <Sprite v-if="sprites && o.value" :id="o.sprite ?? o.value" :size="28" />
          <span class="label">{{ o.label }}</span>
          <small v-if="o.hint">{{ o.hint }}</small>
          <TypeBadge v-if="o.type" :type="o.type" class="mini-type" />
          <CategoryIcon v-if="o.category" :cat="o.category" />
        </li>
        <li v-if="!filtered.length" class="empty">Aucun résultat</li>
      </ul>
    </Teleport>
  </div>
</template>

<style scoped>
.combo {
  position: relative;
  display: flex;
  align-items: center;
  min-width: 0;
}

.field {
  width: 100%;
  min-width: 0;
  padding: 9px 34px 9px 12px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: color-mix(in srgb, var(--text) 7%, transparent);
  color: var(--text);
  font: inherit;
  font-size: 14px;
  font-weight: 600;
  outline: none;
  text-overflow: ellipsis;
  cursor: pointer;
}

.field.has-lead {
  padding-left: 40px;
}

.open .field,
.field:focus {
  border-color: var(--accent-2);
  cursor: text;
}

.disabled .field {
  opacity: 0.55;
  cursor: not-allowed;
}

.lead {
  position: absolute;
  left: 8px;
  pointer-events: none;
}

.chevron {
  position: absolute;
  right: 10px;
  width: 16px;
  height: 16px;
  fill: none;
  stroke: currentColor;
  stroke-width: 2;
  opacity: 0.7;
  pointer-events: none;
}
</style>

<style>
.combo-list {
  position: fixed;
  z-index: 210;
  max-height: 300px;
  margin: 0;
  padding: 4px;
  overflow-y: auto;
  list-style: none;
  border: 1px solid var(--border);
  border-radius: 12px;
  background: var(--surface);
  color: var(--text);
  box-shadow: 0 14px 36px rgba(0, 0, 0, 0.35);
}

.combo-list.above {
  transform: translateY(-100%);
}

.combo-list li {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 10px;
  border-radius: 8px;
  font-size: 14px;
  cursor: pointer;
}

.combo-list li .label {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.combo-list li small {
  color: var(--text-dim);
  font-size: 12px;
}

.combo-list li.active {
  background: color-mix(in srgb, var(--accent-2) 22%, transparent);
}

.combo-list li.good {
  color: var(--ok);
  background: color-mix(in srgb, #3ccf7a 12%, transparent);
}

.combo-list li.good + li:not(.good) {
  margin-top: 6px;
}

.combo-list li .mini-type {
  min-width: 52px;
  font-size: 10px;
}

.combo-list li.chosen .label {
  font-weight: 700;
}

.combo-list li.empty {
  color: var(--text-dim);
  cursor: default;
}
</style>
