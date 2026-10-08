<script setup lang="ts">
import { computed } from "vue";

/** Barre de PV (verte, orange sous la moitié, rouge sous 20 %) et PV en chiffres. */
const props = defineProps<{ hp: number; max: number }>();

const ratio = computed(() => (props.max ? Math.min(1, props.hp / props.max) : 0));
const tone = computed(() => (ratio.value > 0.5 ? "ok" : ratio.value > 0.2 ? "warn" : "danger"));
</script>

<template>
  <span class="hp">
    <span class="bar" :class="tone" role="meter" :aria-valuenow="hp" aria-valuemin="0" :aria-valuemax="max" :aria-label="`PV ${hp} sur ${max}`">
      <i :style="{ width: `${Math.round(ratio * 100)}%` }" />
    </span>
    <small class="num">{{ hp }} / {{ max }}</small>
  </span>
</template>

<style scoped>
.hp {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
}

.bar {
  flex: 1;
  height: 6px;
  border-radius: var(--radius-pill);
  background: color-mix(in srgb, var(--text) 16%, transparent);
  overflow: hidden;
}

.bar i {
  display: block;
  height: 100%;
  border-radius: inherit;
  transition: width 0.4s ease;
}

.bar.ok i {
  background: var(--ok);
}

.bar.warn i {
  background: var(--warn);
}

.bar.danger i {
  background: var(--danger);
}

.num {
  min-width: 64px;
  color: var(--text-dim);
  font-size: var(--fs-sm);
  font-variant-numeric: tabular-nums;
  text-align: right;
}
</style>
