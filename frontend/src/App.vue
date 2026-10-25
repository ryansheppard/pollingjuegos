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
const tab = ref<'ballot' | 'results'>('ballot')
const openSlot = ref<number | null>(null)
const searchInput = ref<HTMLInputElement | null>(null)
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
const nextWeek = computed(() => {
  const index = weeks.value.findIndex((w) => `${w.season}/${w.week}` === selected.value)
  return index > 0 ? weeks.value[index - 1] : undefined
})
// oxlint-disable-next-line no-unused-vars -- Used in the Vue template.
function stepWeek(target: Week | undefined) {
  if (target && !loading.value && !saving.value) selected.value = `${target.season}/${target.week}`
}
// oxlint-disable-next-line no-unused-vars -- Used in the Vue template.
const closesLabel = computed(() => {
  if (!week.value?.closes_at) return 'No deadline'
  return new Date(week.value.closes_at).toLocaleString(undefined, { weekday: 'short', month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit' })
})
// oxlint-disable-next-line no-unused-vars -- Used in the Vue template.
const timeLeft = computed(() => {
  if (!week.value) return '—'
  if (closed.value) return 'Closed'
  if (!week.value.closes_at) return 'Open'
  const minutes = Math.max(0, Math.floor((Date.parse(week.value.closes_at) - now.value) / 60_000))
  const d = Math.floor(minutes / 1440), h = Math.floor((minutes % 1440) / 60), m = minutes % 60
  return d ? `${d}d ${h}h ${String(m).padStart(2, '0')}m` : h ? `${h}h ${String(m).padStart(2, '0')}m` : `${m}m`
})
const crowdRanks = computed(() => new Map((rankings.value?.rankings ?? []).map((row) => [row.quarterback.id, row.rank])))
// oxlint-disable-next-line no-unused-vars -- Used in the Vue template.
function focusSearch() {
  searchInput.value?.scrollIntoView({ behavior: 'smooth', block: 'start' })
  searchInput.value?.focus({ preventScroll: true })
}
// oxlint-disable-next-line no-unused-vars -- Used in the Vue template.
function removeAt(index: number) {
  ballot.value.splice(index, 1)
  openSlot.value = null
  message.value = ''
}
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
  if (openSlot.value === index) openSlot.value = index + direction
  message.value = ''
}

async function loadWeek() {
  const current = week.value
  const id = ++loadId
  rankings.value = null
  savedIds.value = []
  ballot.value = []
  openSlot.value = null
  error.value = ''
  message.value = ''
  if (!current) return
  // A closed week has nothing left to vote on, so lead with its results.
  tab.value = closed.value ? 'results' : 'ballot'
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
    // Weeks arrive newest first; default to the oldest still accepting votes.
    // If all weeks are closed, show the most recent results instead.
    const openWeeks = weekList.filter((w) => !w.is_closed &&
      (!w.closes_at || Date.parse(w.closes_at) > Date.now()))
    const chronological = (a: Week, b: Week) => a.season - b.season || a.week - b.week
    const initialWeek = openWeeks.sort(chronological)[0] ?? [...weekList].sort(chronological).at(-1)
    if (initialWeek) selected.value = `${initialWeek.season}/${initialWeek.week}`
    else loading.value = false
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Could not load poll data.'
    loading.value = false
  }
})
</script>

<template>
  <header class="site-header">
    <div class="bar">
      <div class="brand"><span class="brand-mark" aria-hidden="true">PJ</span><span>Polling<span class="brand-accent">Juegos</span></span></div>
      <nav class="week-stepper" aria-label="Week">
        <button type="button" class="step-button" aria-label="Previous week" :disabled="!previousWeek || loading || saving" @click="stepWeek(previousWeek)">
          <svg aria-hidden="true" viewBox="0 0 24 24"><path d="M15 18l-6-6 6-6" /></svg>
        </button>
        <label class="visually-hidden" for="week">Select week</label>
        <select id="week" v-model="selected" :disabled="loading || saving || !weeks.length">
          <option v-for="w in weeks" :key="`${w.season}/${w.week}`" :value="`${w.season}/${w.week}`">
            {{ w.season }} · Week {{ w.week }}
          </option>
        </select>
        <button type="button" class="step-button" aria-label="Next week" :disabled="!nextWeek || loading || saving" @click="stepWeek(nextWeek)">
          <svg aria-hidden="true" viewBox="0 0 24 24"><path d="M9 18l6-6-6-6" /></svg>
        </button>
      </nav>
      <div class="header-actions">
        <button type="button" class="header-icon" :aria-label="theme === 'dark' ? 'Switch to light mode' : 'Switch to night mode'" :aria-pressed="theme === 'dark'" @click="toggleTheme">
          <svg v-if="theme === 'dark'" aria-hidden="true" viewBox="0 0 24 24"><circle cx="12" cy="12" r="4" /><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4" /></svg>
          <svg v-else aria-hidden="true" viewBox="0 0 24 24"><path d="M21 12.8A9 9 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8z" /></svg>
        </button>
        <a class="sign-out" href="/accounts/logout/">Sign out</a>
      </div>
    </div>
  </header>

  <section class="band" aria-label="This week">
    <div class="band-inner">
      <div class="band-top">
        <div>
          <p class="eyebrow">The smoljuegos QB poll</p>
          <h1>Who’s the GOAT<span class="period">?</span></h1>
        </div>
        <dl v-if="week" class="stats">
          <div><dt>{{ closed ? 'Voting closed' : 'Voting closes' }}</dt><dd>{{ closesLabel }}</dd></div>
          <div><dt>Time left</dt><dd class="stat-accent">{{ timeLeft }}</dd></div>
          <div><dt>Ballots in</dt><dd>{{ rankings?.ballots ?? '—' }}</dd></div>
        </dl>
      </div>
      <div v-if="weeks.length" class="tabs" role="tablist" aria-label="View">
        <button id="tab-ballot" type="button" role="tab" class="tab" :aria-selected="tab === 'ballot'" aria-controls="panel-ballot" @click="tab = 'ballot'">
          Your ballot <span class="tab-chip">{{ ballot.length }}/15</span>
        </button>
        <button id="tab-results" type="button" role="tab" class="tab" :aria-selected="tab === 'results'" aria-controls="panel-results" @click="tab = 'results'">
          <span>Results<span class="tab-extra"> · Top 10</span></span>
        </button>
      </div>
    </div>
  </section>

  <main>
    <p v-if="error" class="notice error" role="alert">{{ error }}</p>
    <p v-if="message" class="notice success" role="status">{{ message }}</p>
    <p v-if="loading" class="notice loading" role="status">Loading the field…</p>
    <p v-else-if="!weeks.length" class="notice loading">No weeks have been created yet.</p>

    <div v-else-if="tab === 'ballot'" id="panel-ballot" class="layout" role="tabpanel" aria-labelledby="tab-ballot">
      <section class="panel ballot-panel" aria-labelledby="ballot-title">
        <div class="panel-heading">
          <div>
            <h2 id="ballot-title">Your top 15</h2>
            <p class="panel-sub">{{ closed ? 'Voting is closed. Your saved ballot is read-only.' : 'Points per slot on the right. Your draft autosaves on this device.' }}</p>
          </div>
          <button v-if="previousWeek && !closed" type="button" class="secondary-button" :disabled="saving" @click="copyPrevious">
            <svg aria-hidden="true" viewBox="0 0 24 24"><rect x="9" y="9" width="12" height="12" rx="2" /><path d="M5 15V5a2 2 0 0 1 2-2h10" /></svg>
            Copy last week
          </button>
        </div>

        <ol class="slot-list">
          <li v-for="(qb, index) in ballot" :key="qb.id" class="slot" :class="{ 'is-open': openSlot === index }">
            <div class="slot-row">
              <span class="slot-number" :class="{ podium: index < 3 }" aria-hidden="true">{{ String(index + 1).padStart(2, '0') }}</span>
              <span class="slot-name">
                <span class="player-name">{{ qb.name }}</span>
                <span class="slot-meta">
                  <span v-if="qb.team" class="team">{{ qb.team.abbreviation }}</span>
                  <span v-if="rankings?.ballots" class="crowd-inline">· {{ crowdRanks.get(qb.id) ? `Crowd #${crowdRanks.get(qb.id)}` : 'Unranked' }}</span>
                </span>
              </span>
              <span v-if="rankings?.ballots" class="crowd-chip" :class="{ unranked: !crowdRanks.get(qb.id) }">{{ crowdRanks.get(qb.id) ? `Crowd #${crowdRanks.get(qb.id)}` : 'Unranked' }}</span>
              <span class="slot-points">{{ 15 - index }} pts</span>
              <button v-if="!closed" type="button" class="icon-button slot-toggle" :aria-expanded="openSlot === index" :aria-label="`Edit ${qb.name}`" @click="openSlot = openSlot === index ? null : index">
                <svg aria-hidden="true" viewBox="0 0 24 24"><path d="M8 6h13M8 12h13M8 18h13M3 6h.01M3 12h.01M3 18h.01" /></svg>
              </button>
              <div v-if="!closed" class="slot-controls">
                <button type="button" class="icon-button" :disabled="saving || index === 0" :aria-label="`Move ${qb.name} up`" @click="move(index, -1)">
                  <svg aria-hidden="true" viewBox="0 0 24 24"><path d="M18 15l-6-6-6 6" /></svg><span class="control-label">Move up</span>
                </button>
                <button type="button" class="icon-button" :disabled="saving || index === ballot.length - 1" :aria-label="`Move ${qb.name} down`" @click="move(index, 1)">
                  <svg aria-hidden="true" viewBox="0 0 24 24"><path d="M6 9l6 6 6-6" /></svg><span class="control-label">Move down</span>
                </button>
                <button type="button" class="icon-button remove-button" :disabled="saving" :aria-label="`Remove ${qb.name}`" @click="removeAt(index)">
                  <svg aria-hidden="true" viewBox="0 0 24 24"><path d="M18 6L6 18M6 6l12 12" /></svg><span class="control-label">Remove</span>
                </button>
              </div>
            </div>
          </li>
          <li v-for="slot in 15 - ballot.length" :key="`empty-${slot}`" class="slot empty">
            <div class="slot-row">
              <span class="slot-number" aria-hidden="true">{{ String(ballot.length + slot).padStart(2, '0') }}</span>
              <span class="open-slot"><span>Open slot<span class="open-slot-extra"> — add a quarterback from the pool</span></span></span>
              <span class="slot-points">{{ 15 - ballot.length - slot + 1 }} pts</span>
            </div>
          </li>
        </ol>

        <div class="save-bar">
          <div class="ticks" role="progressbar" :aria-valuenow="ballot.length" aria-valuemin="0" aria-valuemax="15" aria-label="Ballot picks selected">
            <span v-for="n in 15" :key="n" :class="{ filled: n <= ballot.length }"></span>
          </div>
          <div class="save-row">
            <p class="save-hint">{{ ballot.length === 15 ? 'Lineup complete. Ready to save.' : `${15 - ballot.length} more ${15 - ballot.length === 1 ? 'pick' : 'picks'} to complete your ballot.` }}</p>
            <div class="save-actions">
              <button v-if="!closed && ballot.length < 15" type="button" class="secondary-button add-jump" @click="focusSearch">
                <svg aria-hidden="true" viewBox="0 0 24 24"><path d="M12 5v14M5 12h14" /></svg> Add QB
              </button>
              <button type="button" class="save-button" :disabled="ballot.length !== 15 || saving || closed" @click="submit">
                {{ saving ? 'Saving…' : closed ? 'Voting closed' : 'Save ballot' }}
                <svg aria-hidden="true" viewBox="0 0 24 24"><path d="M5 12h14M13 6l6 6-6 6" /></svg>
              </button>
            </div>
          </div>
        </div>
      </section>

      <section class="panel pool-panel" aria-labelledby="add-title">
        <div class="pool-head">
          <div class="pool-title">
            <h2 id="add-title">The pool</h2>
            <span class="pool-count">{{ available.length }} available</span>
          </div>
          <label class="field-label" for="search">Search by name or team</label>
          <div class="search-wrap">
            <svg aria-hidden="true" viewBox="0 0 24 24"><circle cx="11" cy="11" r="7" /><path d="M20 20l-3.5-3.5" /></svg>
            <input id="search" ref="searchInput" v-model="search" type="search" placeholder="Allen, BUF, Bills…" />
          </div>
        </div>
        <p v-if="!available.length" class="empty-state">No match. Don't see your quarterback? Too bad.</p>
        <ul v-else class="pool-list">
          <li v-for="qb in available" :key="qb.id">
            <span class="team-badge">{{ qb.team?.abbreviation ?? 'FA' }}</span>
            <span class="player-name">{{ qb.name }}</span>
            <button type="button" class="add-button" :disabled="ballot.length >= 15 || saving || closed" :aria-label="`Add ${qb.name}`" @click="addQuarterback(qb)">
              <svg aria-hidden="true" viewBox="0 0 24 24"><path d="M12 5v14M5 12h14" /></svg> Add
            </button>
          </li>
        </ul>
      </section>
    </div>

    <div v-else id="panel-results" class="layout" role="tabpanel" aria-labelledby="tab-results">
      <section class="panel results-panel" aria-labelledby="rankings-title">
        <div class="panel-heading">
          <div>
            <h2 id="rankings-title">The top 10</h2>
            <p class="panel-sub">{{ rankings?.ballots ?? 0 }} {{ rankings?.ballots === 1 ? 'ballot' : 'ballots' }} · {{ closed ? 'final' : 'live until voting closes' }}</p>
          </div>
          <button v-if="rankings?.rankings.length" type="button" class="secondary-button" @click="shareResults">
            <svg aria-hidden="true" viewBox="0 0 24 24"><path d="M4 12v7a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-7M16 6l-4-4-4 4M12 2v13" /></svg>
            Share image
          </button>
        </div>
        <div v-if="rankings?.rankings.length" class="table-scroll">
          <table class="rankings-table">
            <thead>
              <tr>
                <th scope="col" class="col-rank">Rk</th>
                <th v-if="previousWeek" scope="col" class="col-move">Chg</th>
                <th scope="col">Quarterback</th>
                <th scope="col" class="col-points">Points</th>
                <th scope="col" class="col-votes">Ballots</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="entry in rankings.rankings" :key="entry.quarterback.id" :class="{ leader: entry.rank === 1 }">
                <td class="col-rank rank-number">{{ entry.rank }}</td>
                <td v-if="previousWeek" class="col-move">
                  <span v-if="entry.previous_rank === null" class="move move-new">NEW</span>
                  <span v-else-if="entry.previous_rank > entry.rank" class="move move-up" :aria-label="`Up ${entry.previous_rank - entry.rank} from ${entry.previous_rank}`">▲ {{ entry.previous_rank - entry.rank }}</span>
                  <span v-else-if="entry.previous_rank < entry.rank" class="move move-down" :aria-label="`Down ${entry.rank - entry.previous_rank} from ${entry.previous_rank}`">▼ {{ entry.rank - entry.previous_rank }}</span>
                  <span v-else class="move move-steady" aria-label="No change">—</span>
                </td>
                <td><span class="player-name">{{ entry.quarterback.name }}</span><span v-if="entry.quarterback.team" class="team">{{ entry.quarterback.team.abbreviation }}</span></td>
                <td class="col-points">
                  <div class="points-cell">
                    <div class="score-track" aria-hidden="true"><div class="score-fill" :style="{ width: `${entry.points / topScore * 100}%` }"></div></div>
                    <strong>{{ entry.points }}</strong>
                  </div>
                </td>
                <td class="col-votes">{{ entry.votes }}/{{ rankings.ballots }}</td>
              </tr>
            </tbody>
          </table>
        </div>
        <p v-else class="empty-state padded">No votes yet. Be the first to make your picks.</p>
      </section>

      <aside class="sidebar">
        <section v-if="savedIds.length && disagreements.length" class="panel side-card" aria-labelledby="vs-title">
          <h3 id="vs-title">You vs. the crowd</h3>
          <p class="panel-sub">Your saved picks compared with the live top 10.</p>
          <ul class="vs-list">
            <li v-for="row in disagreements" :key="row.qb!.id">
              <span class="player-name">{{ row.qb!.name }}</span>
              <span class="vs-ranks">You <strong>#{{ row.mine }}</strong> · Crowd <strong>{{ row.theirs ? `#${row.theirs}` : '—' }}</strong></span>
            </li>
          </ul>
        </section>
        <section class="panel side-card" aria-labelledby="how-title">
          <h3 id="how-title">How scoring works</h3>
          <p class="methodology">Each ballot ranks 15 quarterbacks. A No. 1 pick earns 15 points, a No. 2 earns 14, and so on down to 1 point for No. 15. The table shows the top 10 by total points; tied scores are ordered by ballot appearances, then name.</p>
        </section>
      </aside>
    </div>
  </main>
  <footer>PollingJuegos <span aria-hidden="true">·</span> Every pick counts.</footer>
</template>
