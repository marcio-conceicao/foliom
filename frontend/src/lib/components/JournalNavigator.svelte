<script lang="ts">
  // Month-grid calendar. Clicking a day navigates to `#/journals/YYYY-MM-DD`.
  // "Hoje" hits /api/journals/today and reads the resolved page name from
  // the post-redirect response.url; we then translate the page name shape
  // (`YYYY_MM_DD` per Logseq journals convention) to the router shape
  // (`YYYY-MM-DD`) and navigate.

  interface Props {
    /** "YYYY-MM" — pins the initially-displayed month. Defaults to today. */
    initialMonth?: string;
  }
  let { initialMonth }: Props = $props();

  function todayMonth(): { year: number; month: number } {
    const now = new Date();
    return { year: now.getFullYear(), month: now.getMonth() }; // month 0-11
  }

  function parseInitial(s: string | undefined): { year: number; month: number } {
    if (!s) return todayMonth();
    const m = /^(\d{4})-(\d{2})$/.exec(s);
    if (!m) return todayMonth();
    return { year: Number(m[1]), month: Number(m[2]) - 1 };
  }

  // Capture the initial month into a non-reactive snapshot. We deliberately
  // don't react to later `initialMonth` prop changes — once mounted, the user
  // navigates via prev/next/today buttons, not via parent prop mutation.
  // svelte-ignore state_referenced_locally
  const initial = parseInitial(initialMonth);
  let year = $state(initial.year);
  let month = $state(initial.month); // 0-11

  const monthLabel = $derived(
    new Date(year, month, 1).toLocaleDateString('pt-BR', { month: 'long', year: 'numeric' }),
  );

  function daysInMonth(y: number, m: number): number {
    return new Date(y, m + 1, 0).getDate();
  }
  function pad2(n: number): string {
    return n < 10 ? '0' + n : String(n);
  }
  function dateStr(y: number, m: number, d: number): string {
    return `${y}-${pad2(m + 1)}-${pad2(d)}`;
  }

  const days = $derived(
    Array.from({ length: daysInMonth(year, month) }, (_, i) => ({
      d: i + 1,
      iso: dateStr(year, month, i + 1),
    })),
  );

  // First-day weekday (0=Sun) — used to pad the grid at the start so the
  // numbers line up under their weekday columns.
  const firstWeekday = $derived(new Date(year, month, 1).getDay());

  function step(delta: number): void {
    let m = month + delta;
    let y = year;
    while (m < 0) {
      m += 12;
      y--;
    }
    while (m > 11) {
      m -= 12;
      y++;
    }
    month = m;
    year = y;
  }

  function navigateTo(iso: string): void {
    window.location.hash = `#/journals/${iso}`;
  }

  async function openToday(): Promise<void> {
    try {
      const res = await fetch('/api/journals/today', { headers: { Accept: 'application/json' } });
      // The backend replies 302 → /api/pages/YYYY_MM_DD; fetch follows it by
      // default and exposes the final URL via response.url.
      const match = /\/api\/pages\/([^/?#]+)/.exec(res.url ?? '');
      if (!match) return;
      const pageName = decodeURIComponent(match[1]);
      // Convert YYYY_MM_DD → YYYY-MM-DD for the journal route.
      const iso = pageName.replace(/_/g, '-');
      navigateTo(iso);
    } catch {
      // Silent — failure surfaces in the page-level RedirectToday if user
      // also lands there. Sidebar's "Hoje" is a convenience.
    }
  }
</script>

<div class="journal-nav">
  <div class="header">
    <button type="button" data-role="prev-month" onclick={() => step(-1)} aria-label="Mês anterior">‹</button>
    <span data-role="month-label">{monthLabel}</span>
    <button type="button" data-role="next-month" onclick={() => step(1)} aria-label="Próximo mês">›</button>
  </div>
  <button type="button" class="today" data-role="today" onclick={openToday}>Hoje</button>
  <div class="grid" role="grid" aria-label={monthLabel}>
    {#each Array(firstWeekday) as _, _i (`pad-${_i}`)}
      <span class="pad"></span>
    {/each}
    {#each days as day (day.iso)}
      <button
        type="button"
        class="day"
        data-date={day.iso}
        onclick={() => navigateTo(day.iso)}
      >{day.d}</button>
    {/each}
  </div>
</div>

<style>
  .journal-nav {
    border: 1px solid var(--guide-color);
    border-radius: 0.3rem;
    padding: 0.4rem;
    margin-bottom: 0.4rem;
  }
  .header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.3rem;
    font-size: 0.85rem;
    margin-bottom: 0.3rem;
  }
  .header button {
    background: none;
    border: 1px solid var(--guide-color);
    color: var(--fg);
    border-radius: 0.25rem;
    padding: 0 0.4rem;
    cursor: pointer;
    line-height: 1.3;
  }
  .header button:hover {
    background: var(--code-bg);
  }
  .today {
    width: 100%;
    background: var(--tag-bg);
    color: var(--tag-fg);
    border: 0;
    border-radius: 0.25rem;
    padding: 0.2rem 0;
    cursor: pointer;
    font-size: 0.82rem;
    margin-bottom: 0.3rem;
  }
  .today:hover {
    filter: brightness(1.05);
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    gap: 0.1rem;
  }
  .pad {
    height: 1.4rem;
  }
  .day {
    background: none;
    border: 1px solid transparent;
    color: var(--fg);
    border-radius: 0.2rem;
    padding: 0.1rem 0;
    cursor: pointer;
    font-size: 0.78rem;
    line-height: 1.2;
    text-align: center;
  }
  .day:hover {
    background: var(--code-bg);
    border-color: var(--guide-color);
  }
</style>
