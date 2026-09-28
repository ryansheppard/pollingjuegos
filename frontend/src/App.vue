<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
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
const theme = ref<'light' | 'dark'>(getInitialTheme())
const userId = ref<number | null>(null)
const savedIds = ref<number[]>([])
const now = ref(Date.now())
let clock: ReturnType<typeof setInterval> | undefined
document.documentElement.dataset.theme = theme.value
let loadId = 0

function getInitialTheme(): 'light' | 'dark' {
  try {
    const saved = localStorage.getItem('pollingjuegos-theme')
    if (saved === 'light' || saved === 'dark') return saved
  } catch { /* Storage can be disabled; the toggle still works for this visit. */ }
  return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'
}

function toggleTheme() {
  theme.value = theme.value === 'dark' ? 'light' : 'dark'
  document.documentElement.dataset.theme = theme.value
  try {
    localStorage.setItem('pollingjuegos-theme', theme.value)
  } catch { /* Keep the in-memory choice if storage is unavailable. */ }
}

const week = computed(() => weeks.value.find((w) => `${w.season}/${w.week}` === selected.value))
const closed = computed(() => !!week.value && (week.value.is_closed || (!!week.value.closes_at && now.value >= Date.parse(week.value.closes_at))))
const previousWeek = computed(() => {
  const index = weeks.value.findIndex((w) => `${w.season}/${w.week}` === selected.value)
  return index < 0 ? undefined : weeks.value[index + 1]
})
const draftKey = computed(() => userId.value === null || !week.value ? null : `pollingjuegos-draft-${userId.value}-${week.value.season}-${week.value.week}`)
const disagreements = computed(() => {
  if (!savedIds.value.length || !rankings.value?.rankings.length) return []
  const ranks = new Map(rankings.value.rankings.map((row) => [row.quarterback.id, row.rank]))
  const byId = new Map(catalog.value.map((qb) => [qb.id, qb]))
  return savedIds.value.map((id, index) => ({ qb: byId.get(id), mine: index + 1, theirs: ranks.get(id) }))
    .filter((row) => row.qb && row.theirs !== row.mine)
    .sort((a, b) => Math.abs(b.mine - (b.theirs ?? 11)) - Math.abs(a.mine - (a.theirs ?? 11))).slice(0, 3)
})
function sameIds(a: number[], b: number[]) { return a.length === b.length && a.every((id, index) => id === b[index]) }
function draftStorage() { try { return window.localStorage } catch { return null } }
watch(ballot, () => {
  const key = draftKey.value
  if (loading.value || !key || closed.value) return
  const ids = ballot.value.map((qb) => qb.id)
  try {
    if (sameIds(ids, savedIds.value)) draftStorage()?.removeItem(key)
    else draftStorage()?.setItem(key, JSON.stringify(ids))
  } catch { /* Private browsing or full storage: ballot still works without draft recovery. */ }
}, { deep: true })
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
  savedIds.value = []
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
    savedIds.value = mine.entries.map((entry) => entry.quarterback.id)
    let ids = savedIds.value
    const key = draftKey.value
    if (key && !closed.value) {
      try {
        const stored: unknown = JSON.parse(draftStorage()?.getItem(key) ?? 'null')
        const known = new Set(catalog.value.map((qb) => qb.id))
        if (Array.isArray(stored) && stored.length <= 15 && new Set(stored).size === stored.length &&
            stored.every((value) => Number.isInteger(value) && known.has(value))) {
          ids = stored as number[]
          if (!sameIds(ids, savedIds.value)) message.value = 'Your unfinished draft was restored on this device.'
        } else if (stored !== null) draftStorage()?.removeItem(key)
      } catch { /* Ignore invalid or unavailable local drafts. */ }
    }
    const byId = new Map(catalog.value.map((qb) => [qb.id, qb]))
    ballot.value = ids.map((qbId) => byId.get(qbId)!).filter((qb) => !!qb)
  } catch (err) {
    if (id === loadId) error.value = err instanceof Error ? err.message : 'Could not load this week.'
  } finally {
    if (id === loadId) loading.value = false
  }
}

// oxlint-disable-next-line no-unused-vars -- Used in the Vue template.
async function submit() {
  const current = week.value
  if (!current || ballot.value.length !== 15 || saving.value || closed.value) return
  saving.value = true
  error.value = ''
  message.value = ''
  try {
    const result = await saveBallot(current, ballot.value.map((qb) => qb.id))
    ballot.value = result.entries.map((entry) => entry.quarterback)
    savedIds.value = ballot.value.map((qb) => qb.id)
    try { if (draftKey.value) draftStorage()?.removeItem(draftKey.value) } catch { /* Optional draft storage. */ }
    message.value = 'Ballot saved. Horrible picks.'
    try {
      rankings.value = await request<Rankings>(`/weeks/${current.season}/${current.week}/rankings`)
    } catch {
      error.value = 'Ballot saved, but rankings could not be refreshed. Reload to try again.'
    }
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Could not save ballot.'
    if (error.value.includes('closed')) { now.value = Date.now(); if (week.value) week.value.is_closed = true }
  } finally {
    saving.value = false
  }
}

async function copyPrevious() {
  const prev = previousWeek.value
  if (!prev || closed.value || saving.value) return
  if (!sameIds(ballot.value.map((qb) => qb.id), savedIds.value) &&
      !window.confirm('Replace your unfinished changes with last week’s ballot?')) return
  saving.value = true
  error.value = ''
  try {
    const last = await request<Ballot>(`/weeks/${prev.season}/${prev.week}/ballot`)
    if (!last.entries.length) { message.value = 'You did not submit a ballot last week.'; return }
    const byId = new Map(catalog.value.map((qb) => [qb.id, qb]))
    ballot.value = last.entries.map((entry) => byId.get(entry.quarterback.id)).filter((qb): qb is Quarterback => !!qb)
    message.value = 'Copied last week’s picks. Review and save to submit this week.'
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Could not copy last week’s ballot.'
  } finally { saving.value = false }
}

async function shareResults() {
  if (!week.value || !rankings.value?.rankings.length) return
  try {
    const canvas = document.createElement('canvas')
    canvas.width = 1000; canvas.height = 210 + rankings.value.rankings.length * 58
    const ctx = canvas.getContext('2d')
    if (!ctx) throw new Error('Image export is not supported by this browser.')
    ctx.fillStyle = '#122b46'; ctx.fillRect(0, 0, canvas.width, canvas.height)
    ctx.fillStyle = '#f09866'; ctx.font = 'bold 28px sans-serif'; ctx.fillText('POLLINGJUEGOS', 48, 62)
    ctx.fillStyle = '#fff'; ctx.font = 'bold 44px sans-serif'; ctx.fillText(`${week.value.season} · WEEK ${week.value.week} TOP 10`, 48, 126)
    ctx.font = '20px sans-serif'; ctx.fillStyle = '#b3bfcb'; ctx.fillText(`${rankings.value.ballots} ballots · 15 points for #1 through 1 for #15`, 48, 166)
    rankings.value.rankings.forEach((row, i) => {
      const y = 226 + i * 58
      ctx.fillStyle = '#f09866'; ctx.font = 'bold 29px sans-serif'; ctx.fillText(String(row.rank).padStart(2, '0'), 48, y)
      ctx.fillStyle = '#fff'; ctx.font = '25px sans-serif'; ctx.fillText(`${row.quarterback.name}  ${row.quarterback.team?.abbreviation ?? ''}`, 112, y)
      ctx.textAlign = 'right'; ctx.fillText(`${row.points} pts`, 952, y); ctx.textAlign = 'left'
    })
    const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, 'image/png'))
    if (!blob) throw new Error('Could not create the results image.')
    const name = `pollingjuegos-${week.value.season}-week-${week.value.week}.png`
    const file = new File([blob], name, { type: 'image/png' })
    if (navigator.share && navigator.canShare?.({ files: [file] })) {
      try { await navigator.share({ files: [file], title: 'PollingJuegos results' }); return }
      catch (err) { if (err instanceof Error && err.name === 'AbortError') return }
    }
    const url = URL.createObjectURL(blob)
    const link = document.createElement('a'); link.href = url; link.download = name; link.click()
    setTimeout(() => URL.revokeObjectURL(url), 60_000)
  } catch (err) { error.value = err instanceof Error ? err.message : 'Could not export results.' }
}

watch(selected, loadWeek)
onUnmounted(() => { if (clock) clearInterval(clock) })
onMounted(async () => {
  clock = setInterval(() => { now.value = Date.now() }, 30_000)
  try {
    const [weekList, qbs, me] = await Promise.all([request<Week[]>('/weeks'), loadQuarterbacks(), request<{ id: number }>('/me')])
    userId.value = me.id
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
      <div class="header-actions">
        <button type="button" class="theme-toggle" :aria-label="theme === 'dark' ? 'Switch to light mode' : 'Switch to night mode'" :aria-pressed="theme === 'dark'" @click="toggleTheme">
          <span aria-hidden="true">{{ theme === 'dark' ? '☀' : '☾' }}</span> {{ theme === 'dark' ? 'Light mode' : 'Night mode' }}
        </button>
        <a class="sign-out" href="/accounts/logout/">Sign out <span aria-hidden="true">↗</span></a>
      </div>
    </header>

    <div class="hero">
      <div>
        <p class="eyebrow">The smoljuegos poll / {{ week ? `${week.season} · Week ${week.week}` : 'Weekly rankings' }}</p>
        <h1>Who’s the GOAT<span class="period">?</span></h1>
        <p class="hero-copy">An elite list picked by the greatest minds in a small Discord channel.</p>
        <p v-if="week?.closes_at" class="deadline">{{ closed ? 'Voting closed' : 'Voting closes' }} {{ new Date(week.closes_at).toLocaleString() }}</p>
        <p v-else-if="week" class="deadline">Voting is open · No deadline set</p>
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
          <div class="heading-actions"><button v-if="rankings?.rankings.length" type="button" class="secondary-button" @click="shareResults">Share image</button><span class="pill">{{ rankings?.ballots ?? 0 }} {{ rankings?.ballots === 1 ? 'ballot' : 'ballots' }}</span></div>
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
              <span v-if="previousWeek && entry.previous_rank !== null" class="movement" :aria-label="`Previously ranked ${entry.previous_rank}`">{{ entry.previous_rank > entry.rank ? `↑ ${entry.previous_rank - entry.rank}` : entry.previous_rank < entry.rank ? `↓ ${entry.rank - entry.previous_rank}` : '— steady' }}</span>
              <span v-else-if="previousWeek && rankings?.ballots" class="movement">New to top 10</span>
            </div>
          </li>
        </ol>
        <p v-else class="empty-state">No votes yet. Be the first to make your picks.</p>
        <div v-if="savedIds.length && disagreements.length" class="methodology comparison">
          <strong>Your ballot vs. the crowd</strong>
          <p v-for="row in disagreements" :key="row.qb!.id">{{ row.qb!.name }}: your #{{ row.mine }} vs. {{ row.theirs ? `group #${row.theirs}` : 'outside the top 10' }}</p>
          <p class="comparison-note">Your saved picks compared with the live top 10.</p>
        </div>
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
        <button v-if="previousWeek && !closed" type="button" class="secondary-button copy-button" :disabled="saving" @click="copyPrevious">Copy previous week's ballot</button>
        <p v-if="closed" class="ballot-hint">Voting is closed. Your saved ballot is read-only.</p>
        <div class="progress-track" role="progressbar" :aria-valuenow="ballot.length" aria-valuemin="0" aria-valuemax="15" aria-label="Ballot picks selected">
          <div class="progress-fill" :style="{ width: `${ballot.length / 15 * 100}%` }"></div>
        </div>
        <p class="ballot-hint">{{ ballot.length === 15 ? 'Lineup complete. Ready to save.' : `${15 - ballot.length} more ${15 - ballot.length === 1 ? 'pick' : 'picks'} to complete your ballot.` }}</p>
        <ol v-if="ballot.length" class="ballot-list">
          <li v-for="(qb, index) in ballot" :key="qb.id" class="ballot-item">
            <span class="slot-number" aria-hidden="true">{{ String(index + 1).padStart(2, '0') }}</span>
            <span class="player-name">{{ qb.name }} <span v-if="qb.team" class="team">{{ qb.team.abbreviation }}</span></span>
            <div class="controls">
              <button type="button" class="icon-button" :disabled="saving || closed || index === 0" :aria-label="`Move ${qb.name} up`" @click="move(index, -1)">↑</button>
              <button type="button" class="icon-button" :disabled="saving || closed || index === ballot.length - 1" :aria-label="`Move ${qb.name} down`" @click="move(index, 1)">↓</button>
              <button type="button" class="remove-button" :disabled="saving || closed" :aria-label="`Remove ${qb.name}`" @click="ballot.splice(index, 1); message = ''">×</button>
            </div>
          </li>
        </ol>
        <p v-else class="empty-state">Your board is empty. Add quarterbacks below to get started.</p>
        <button type="button" class="save-button" :disabled="ballot.length !== 15 || saving || closed" @click="submit">
          {{ saving ? 'Saving…' : closed ? 'Voting closed' : 'Save ballot' }} <span aria-hidden="true">→</span>
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
            <button type="button" class="add-button" :disabled="ballot.length >= 15 || saving || closed" :aria-label="`Add ${qb.name}`" @click="addQuarterback(qb)">Add <span aria-hidden="true">+</span></button>
          </li>
        </ul>
      </section>
    </div>
    <footer>PollingJuegos <span aria-hidden="true">·</span> Every pick counts.</footer>
  </main>
</template>
