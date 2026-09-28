<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { loadQuarterbacks, request, saveBallot, type Ballot, type Quarterback, type Rankings, type Week } from './api'

const weeks = ref<Week[]>([])
const selected = ref('')
const catalog = ref<Quarterback[]>([])
const ballot = ref<Quarterback[]>([])
const rankings = ref<Rankings | null>(null)
const search = ref('')
const loading = ref(true)
const saving = ref(false)
const error = ref('')
const message = ref('')
let loadId = 0

const week = computed(() => weeks.value.find((w) => `${w.season}/${w.week}` === selected.value))
const topScore = computed(() => rankings.value?.rankings[0]?.points || 1)
// oxlint-disable-next-line no-unused-vars -- Used in the Vue template.
const available = computed(() => {
  const selectedIds = new Set(ballot.value.map((qb) => qb.id))
  const query = search.value.trim().toLowerCase()
  return catalog.value.filter((qb) => !selectedIds.has(qb.id) &&
    `${qb.name} ${qb.team?.name ?? ''} ${qb.team?.abbreviation ?? ''}`.toLowerCase().includes(query))
})

// oxlint-disable-next-line no-unused-vars -- Used in the Vue template.
function label(qb: Quarterback) {
  return `${qb.name}${qb.team ? ` (${qb.team.abbreviation})` : ''}`
}

// oxlint-disable-next-line no-unused-vars -- Used in the Vue template.
function addQuarterback(qb: Quarterback) {
  if (saving.value || ballot.value.length >= 15 || ballot.value.some((entry) => entry.id === qb.id)) return
  if (qb.name.trim().toLowerCase() === 'cam ward' && !window.confirm('Are you sure?')) return
  ballot.value.push(qb)
  message.value = ''
}

// oxlint-disable-next-line no-unused-vars -- Used in the Vue template.
function move(index: number, direction: number) {
  const entries = [...ballot.value]
  ;[entries[index], entries[index + direction]] = [entries[index + direction]!, entries[index]!]
  ballot.value = entries
  message.value = ''
}

async function loadWeek() {
  const current = week.value
  const id = ++loadId
  rankings.value = null
  ballot.value = []
  error.value = ''
  message.value = ''
  if (!current) return
  loading.value = true
  try {
    const base = `/weeks/${current.season}/${current.week}`
    const [results, mine] = await Promise.all([
      request<Rankings>(`${base}/rankings`),
      request<Ballot>(`${base}/ballot`),
    ])
    if (id !== loadId) return
    rankings.value = results
    ballot.value = mine.entries.map((entry) => entry.quarterback)
  } catch (err) {
    if (id === loadId) error.value = err instanceof Error ? err.message : 'Could not load this week.'
  } finally {
    if (id === loadId) loading.value = false
  }
}

// oxlint-disable-next-line no-unused-vars -- Used in the Vue template.
async function submit() {
  const current = week.value
  if (!current || ballot.value.length !== 15 || saving.value) return
  saving.value = true
  error.value = ''
  message.value = ''
  try {
    const result = await saveBallot(current, ballot.value.map((qb) => qb.id))
    ballot.value = result.entries.map((entry) => entry.quarterback)
    message.value = 'Ballot saved. Horrible picks.'
    try {
      rankings.value = await request<Rankings>(`/weeks/${current.season}/${current.week}/rankings`)
    } catch {
      error.value = 'Ballot saved, but rankings could not be refreshed. Reload to try again.'
    }
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Could not save ballot.'
  } finally {
    saving.value = false
  }
}

watch(selected, loadWeek)
onMounted(async () => {
  try {
    const [weekList, qbs] = await Promise.all([request<Week[]>('/weeks'), loadQuarterbacks()])
    catalog.value = qbs
    weeks.value = weekList
    if (weekList.length) selected.value = `${weekList[0]!.season}/${weekList[0]!.week}`
    else loading.value = false
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Could not load poll data.'
    loading.value = false
  }
})
</script>

<template>
  <main>
    <header class="site-header">
      <div class="brand"><span class="brand-mark" aria-hidden="true">PJ</span><span>Polling<span class="brand-accent">Juegos</span></span></div>
      <a class="sign-out" href="/accounts/logout/">Sign out <span aria-hidden="true">↗</span></a>
    </header>

    <div class="hero">
      <div>
        <p class="eyebrow">The smoljuegos poll / {{ week ? `${week.season} · Week ${week.week}` : 'Weekly rankings' }}</p>
        <h1>Who’s the GOAT<span class="period">?</span></h1>
        <p class="hero-copy">An elite list picked by the greatest minds in a small Discord channel.</p>
      </div>
      <div class="week-picker">
        <label for="week">SELECT WEEK</label>
        <select id="week" v-model="selected" :disabled="loading || saving || !weeks.length">
          <option v-for="w in weeks" :key="`${w.season}/${w.week}`" :value="`${w.season}/${w.week}`">
            {{ w.season }} · Week {{ w.week }}
          </option>
        </select>
      </div>
    </div>

    <p v-if="error" class="notice error" role="alert">{{ error }}</p>
    <p v-if="message" class="notice success" role="status">{{ message }}</p>
    <p v-if="loading" class="notice loading" role="status">Loading the field…</p>
    <p v-else-if="!weeks.length" class="notice loading">No weeks have been created yet.</p>
    <div v-else class="grid">
      <section class="panel rankings-panel" aria-labelledby="rankings-title">
        <div class="panel-heading">
          <div><p class="eyebrow">The results</p><h2 id="rankings-title">The top 10</h2></div>
          <span class="pill">{{ rankings?.ballots ?? 0 }} {{ rankings?.ballots === 1 ? 'ballot' : 'ballots' }}</span>
        </div>
        <p v-if="rankings?.rankings.length" class="chart-label">POINTS <span>BIGGER IS BETTER</span></p>
        <ol v-if="rankings?.rankings.length" class="rankings-list">
          <li v-for="entry in rankings.rankings" :key="entry.quarterback.id" class="ranking-item">
            <span class="rank-number" aria-hidden="true">{{ String(entry.rank).padStart(2, '0') }}</span>
            <div class="ranking-content">
              <div class="ranking-line">
                <span class="player-name">{{ entry.quarterback.name }} <span v-if="entry.quarterback.team" class="team">{{ entry.quarterback.team.abbreviation }}</span></span>
                <strong class="score">{{ entry.points }} <span>pts</span></strong>
              </div>
              <div class="score-track" aria-hidden="true"><div class="score-fill" :style="{ width: `${entry.points / topScore * 100}%` }"></div></div>
              <span class="vote-count">Appeared on {{ entry.votes }} {{ entry.votes === 1 ? 'ballot' : 'ballots' }}</span>
            </div>
          </li>
        </ol>
        <p v-else class="empty-state">No votes yet. Be the first to make your picks.</p>
        <div class="methodology">
          <strong>How this works</strong>
          <p>Each ballot ranks 15 quarterbacks. A No. 1 pick earns 15 points, a No. 2 earns 14, and so on down to 1 point for No. 15. The chart shows the top 10 by total points; tied scores are ordered by ballot appearances, then name.</p>
        </div>
      </section>

      <section class="panel ballot-panel" aria-labelledby="ballot-title">
        <div class="panel-heading">
          <div><p class="eyebrow">Have your say</p><h2 id="ballot-title">Your ballot</h2></div>
          <span class="pill" :class="{ 'pill-complete': ballot.length === 15 }">{{ ballot.length }} / 15</span>
        </div>
        <div class="progress-track" role="progressbar" :aria-valuenow="ballot.length" aria-valuemin="0" aria-valuemax="15" aria-label="Ballot picks selected">
          <div class="progress-fill" :style="{ width: `${ballot.length / 15 * 100}%` }"></div>
        </div>
        <p class="ballot-hint">{{ ballot.length === 15 ? 'Lineup complete. Ready to save.' : `${15 - ballot.length} more ${15 - ballot.length === 1 ? 'pick' : 'picks'} to complete your ballot.` }}</p>
        <ol v-if="ballot.length" class="ballot-list">
          <li v-for="(qb, index) in ballot" :key="qb.id" class="ballot-item">
            <span class="slot-number" aria-hidden="true">{{ String(index + 1).padStart(2, '0') }}</span>
            <span class="player-name">{{ qb.name }} <span v-if="qb.team" class="team">{{ qb.team.abbreviation }}</span></span>
            <div class="controls">
              <button type="button" class="icon-button" :disabled="saving || index === 0" :aria-label="`Move ${qb.name} up`" @click="move(index, -1)">↑</button>
              <button type="button" class="icon-button" :disabled="saving || index === ballot.length - 1" :aria-label="`Move ${qb.name} down`" @click="move(index, 1)">↓</button>
              <button type="button" class="remove-button" :disabled="saving" :aria-label="`Remove ${qb.name}`" @click="ballot.splice(index, 1); message = ''">×</button>
            </div>
          </li>
        </ol>
        <p v-else class="empty-state">Your board is empty. Add quarterbacks below to get started.</p>
        <button type="button" class="save-button" :disabled="ballot.length !== 15 || saving" @click="submit">
          {{ saving ? 'Saving…' : 'Save ballot' }} <span aria-hidden="true">→</span>
        </button>
      </section>

      <section class="panel add-panel" aria-labelledby="add-title">
        <div class="panel-heading"><h2 id="add-title">Add quarterbacks</h2></div>
        <p class="add-help">Don't see your quarterback listed? Too bad.</p>
        <label class="search-label" for="search">Search by name or team</label>
        <input id="search" v-model="search" type="search" placeholder="Search quarterbacks or teams…" />
        <p v-if="!available.length" class="empty-state">No matching quarterbacks.</p>
        <ul v-else class="available-list">
          <li v-for="qb in available" :key="qb.id">
            <span class="player-name">{{ qb.name }} <span v-if="qb.team" class="team">{{ qb.team.abbreviation }}</span></span>
            <button type="button" class="add-button" :disabled="ballot.length >= 15 || saving" :aria-label="`Add ${qb.name}`" @click="addQuarterback(qb)">Add <span aria-hidden="true">+</span></button>
          </li>
        </ul>
      </section>
    </div>
    <footer>PollingJuegos <span aria-hidden="true">·</span> Every pick counts.</footer>
  </main>
</template>
