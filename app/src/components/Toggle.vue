<script setup lang="ts">
defineProps<{ label: string; hint?: string }>();
const model = defineModel<boolean>({ required: true });
</script>

<template>
  <label class="toggle" :title="hint">
    <input v-model="model" type="checkbox" />
    <span class="track"><span class="thumb" /></span>
    <span class="label">{{ label }}</span>
  </label>
</template>

<style scoped>
.toggle {
  display: inline-flex;
  align-items: center;
  gap: 10px;
  cursor: pointer;
}

input {
  position: absolute;
  opacity: 0;
  pointer-events: none;
}

.track {
  position: relative;
  width: 40px;
  height: 22px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--text) 18%, transparent);
  transition: background 0.2s;
}

.thumb {
  position: absolute;
  top: 3px;
  left: 3px;
  width: 16px;
  height: 16px;
  border-radius: 50%;
  background: #fff;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.3);
  transition: transform 0.2s;
}

input:checked + .track {
  background: var(--accent-2);
}

input:checked + .track .thumb {
  transform: translateX(18px);
}

input:focus-visible + .track {
  outline: 2px solid var(--accent);
  outline-offset: 2px;
}

.label {
  font-size: 14px;
}
</style>
