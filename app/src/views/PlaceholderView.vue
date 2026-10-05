<script setup lang="ts">
defineProps<{
  title: string;
  phase: string;
  description: string;
  features: readonly string[];
}>();
</script>

<template>
  <section class="placeholder">
    <span class="chip chip-accent">{{ phase }}</span>
    <h1>{{ title }}</h1>
    <p class="lead">{{ description }}</p>

    <ul class="panel">
      <li v-for="f in features" :key="f">
        <span class="dot" />
        {{ f }}
      </li>
    </ul>

    <p class="hint">En attendant, dépose tes ROMs dans la bibliothèque : elles seront prêtes ici dès que ce module arrivera.</p>
  </section>
</template>

<style scoped>
.placeholder {
  max-width: 720px;
  margin: 40px auto 0;
}

h1 {
  margin-top: 14px;
  font-size: 34px;
}

.lead {
  color: var(--text-dim);
  font-size: 16px;
  line-height: 1.5;
}

ul {
  list-style: none;
  margin: 24px 0;
  padding: 8px;
}

li {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 14px;
  border-radius: var(--radius-sm);
}

li + li {
  border-top: 1px solid var(--border);
}

.dot {
  width: 8px;
  height: 8px;
  flex-shrink: 0;
  border-radius: 50%;
  background: var(--prism);
}

.hint {
  color: var(--text-dim);
  font-size: 13px;
}
</style>
