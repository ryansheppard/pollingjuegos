export interface Quarterback {
  id: number
  name: string
  team: { id: number; name: string; abbreviation: string } | null
}

export interface Week {
  season: number
  week: number
}

export interface Rankings {
  season: number
  week: number
  ballots: number
  rankings: { rank: number; points: number; votes: number; quarterback: Quarterback }[]
}

export interface Ballot {
  season: number
  week: number
  entries: { rank: number; quarterback: Quarterback }[]
}

export class ApiError extends Error {
  constructor(message: string, public status: number) {
    super(message)
  }
}

export async function request<T>(path: string, options?: RequestInit): Promise<T> {
  const response = await fetch(`/api${path}`, { credentials: 'same-origin', ...options })
  if (!response.ok) {
    const body = await response.json().catch(() => null)
    throw new ApiError(body?.detail ?? `Request failed (${response.status})`, response.status)
  }
  return response.json() as Promise<T>
}

export async function loadQuarterbacks(): Promise<Quarterback[]> {
  const all: Quarterback[] = []
  while (true) {
    const page = await request<Quarterback[]>(`/quarterbacks?limit=100&offset=${all.length}`)
    all.push(...page)
    if (page.length < 100) return all
  }
}

export async function saveBallot(week: Week, quarterbackIds: number[]): Promise<Ballot> {
  // Read a fresh token after sign-in; Django may rotate it on login.
  const { csrf_token } = await request<{ csrf_token: string }>('/csrf')
  return request<Ballot>(`/weeks/${week.season}/${week.week}/ballot`, {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json', 'X-CSRFToken': csrf_token },
    body: JSON.stringify({ quarterback_ids: quarterbackIds }),
  })
}
