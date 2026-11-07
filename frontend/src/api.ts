import type { Ballot, QuarterbackWithStats, Week } from './api.gen'

export type * from './api.gen'

export class ApiError extends Error {
  constructor(message: string, public status: number) {
    super(message)
  }
}

export async function request<T>(path: string, options?: RequestInit): Promise<T> {
  const response = await fetch(`/api${path}`, { credentials: 'same-origin', ...options })
  if (response.status === 401) {
    window.location.assign(`/accounts/login/?next=${encodeURIComponent(window.location.pathname + window.location.search)}`)
    throw new ApiError('Sign in required', 401)
  }
  if (!response.ok) {
    const body = await response.json().catch(() => null)
    throw new ApiError(body?.detail ?? `Request failed (${response.status})`, response.status)
  }
  return response.json() as Promise<T>
}

export async function loadQuarterbacks(): Promise<QuarterbackWithStats[]> {
  const all: QuarterbackWithStats[] = []
  while (true) {
    const page = await request<QuarterbackWithStats[]>(`/quarterbacks?limit=100&offset=${all.length}`)
    all.push(...page)
    if (page.length < 100) return all
  }
}

export async function saveBallot(week: Week, quarterbackIds: number[]): Promise<Ballot> {
  return request<Ballot>(`/weeks/${week.season}/${week.week}/ballot`, {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ quarterback_ids: quarterbackIds }),
  })
}
