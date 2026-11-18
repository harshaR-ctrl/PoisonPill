// Popup: renders the last stored report in the extension's own document.
//
// No highlighting, jump-to-clause, mail generation or history. Those are product
// features layered on top of the contract in docs/CONTRACT.md. The bands below
// are display-only; the engine defines no thresholds for `poison_index`.

import { gsap } from 'gsap';

import type { ClauseFinding, ScanRecord } from './types';

/** Same colours, evenly spaced, as the gauge's `#risk` gradient in popup.html. */
const RISK_STOPS = ['#22c55e', '#eab308', '#f97316', '#ef4444', '#be123c'];

type Band = 'none' | 'low' | 'moderate' | 'high' | 'severe';

interface BandInfo {
  band: Band;
  below: number;
  label: string;
  verdict: string;
}

const SEVERE: BandInfo = {
  band: 'severe',
  below: Infinity,
  label: 'Severe risk',
  verdict: 'This agreement heavily restricts your legal rights.',
};

const BANDS: BandInfo[] = [
  { band: 'none', below: Number.EPSILON, label: 'Clean', verdict: 'No predatory clauses were found.' },
  { band: 'low', below: 25, label: 'Low risk', verdict: 'A few clauses limit your legal options.' },
  { band: 'moderate', below: 50, label: 'Moderate risk', verdict: 'Some clauses restrict how you can resolve disputes.' },
  { band: 'high', below: 75, label: 'High risk', verdict: 'Several clauses take away your right to sue.' },
  SEVERE,
];

/** Plain-language names for rule ids; unknown rules fall back to the engine label. */
const RULES: Record<string, { title: string; blurb: string }> = {
  'forced_arbitration.mandatory_arbitration': {
    title: 'Mandatory arbitration',
    blurb: 'Disputes go to a private arbitrator instead of a court.',
  },
  'forced_arbitration.class_action_waiver': {
    title: 'Class action waiver',
    blurb: 'You give up joining others in a group lawsuit.',
  },
  'forced_arbitration.class_action_bar': {
    title: 'Individual claims only',
    blurb: 'Claims may only be brought by you alone, not as a group.',
  },
  'forced_arbitration.jury_trial_waiver': {
    title: 'Jury trial waiver',
    blurb: 'You give up the right to have a jury decide your case.',
  },
  'forced_arbitration.court_access_waiver': {
    title: 'Court access waiver',
    blurb: 'You give up the right to take the company to court.',
  },
};

function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  className?: string,
  text?: string,
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

function show(id: string): void {
  document.getElementById(id)?.removeAttribute('hidden');
}

function setText(id: string, text: string): void {
  const node = document.getElementById(id);
  if (node) node.textContent = text;
}

function bandFor(index: number): BandInfo {
  return BANDS.find((b) => index < b.below) ?? SEVERE;
}

function relativeTime(iso: string): string {
  const seconds = Math.max(0, (Date.now() - Date.parse(iso)) / 1000);
  if (!Number.isFinite(seconds)) return '';
  if (seconds < 60) return 'just now';
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes} min ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours} h ago`;
  return new Date(iso).toLocaleDateString();
}

function hostOf(url: string): string {
  try {
    const { hostname, pathname } = new URL(url);
    return hostname.replace(/^www\./, '') + (pathname === '/' ? '' : pathname);
  } catch {
    return url;
  }
}

function renderFinding(finding: ClauseFinding): HTMLLIElement {
  const item = el('li', 'finding');

  // Clamped to four lines; made expandable only if it actually overflows
  // (see makeExpandable, which runs once the quote is laid out).
  const quote = el('blockquote', 'clamped', finding.raw_text);

  const meta = el('div', 'meta');
  if (finding.opt_out_available) {
    meta.append(
      el(
        'span',
        'badge optout',
        finding.opt_out_target ? `Opt-out: ${finding.opt_out_target}` : 'Opt-out available',
      ),
    );
  } else {
    meta.append(el('span', 'badge', 'No opt-out stated'));
  }

  const why = el('details');
  why.append(el('summary', undefined, 'Why flagged'), el('p', undefined, finding.reason));
  meta.append(why);

  item.append(quote, meta);
  return item;
}

/** Turn a clamped quote into a toggle, or unclamp it if it already fits. */
function makeExpandable(quote: HTMLElement): void {
  if (quote.scrollHeight <= quote.clientHeight + 1) {
    quote.classList.remove('clamped');
    return;
  }
  quote.setAttribute('role', 'button');
  quote.tabIndex = 0;
  quote.setAttribute('aria-expanded', 'false');
  quote.title = 'Show full clause';
  const toggle = () => {
    const expanded = quote.classList.toggle('clamped') === false;
    quote.setAttribute('aria-expanded', String(expanded));
    quote.title = expanded ? 'Show less' : 'Show full clause';
  };
  quote.addEventListener('click', toggle);
  quote.addEventListener('keydown', (event) => {
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      toggle();
    }
  });
}

function renderGroups(findings: ClauseFinding[]): HTMLElement[] {
  const groups = new Map<string, ClauseFinding[]>();
  for (const finding of findings) {
    const list = groups.get(finding.rule_id) ?? [];
    list.push(finding);
    groups.set(finding.rule_id, list);
  }

  return [...groups.entries()]
    .sort((a, b) => b[1].length - a[1].length)
    .map(([ruleId, list]) => {
      const rule = RULES[ruleId] ?? { title: list[0]?.label ?? ruleId, blurb: '' };
      const section = el('section', 'card group');

      const heading = el('h2');
      heading.append(el('span', undefined, rule.title), el('span', 'count', String(list.length)));
      section.append(heading);
      if (rule.blurb) section.append(el('p', 'blurb', rule.blurb));

      const ol = el('ol');
      for (const finding of list) ol.append(renderFinding(finding));
      section.append(ol);
      return section;
    });
}

function formatIndex(index: number): string {
  return index.toFixed(index % 1 ? 1 : 0);
}

/**
 * Score-card entrance: the arc fills, the knob rides its end (taking the
 * gradient's colour at that point) and the number counts up, all together;
 * the risk pill and verdict follow as the sweep settles. Reduced motion jumps
 * straight to the end state.
 */
function animateScore(value: number, index: number): void {
  const fill = document.getElementById('gauge-fill');
  const knob = document.getElementById('gauge-knob');
  const readout = document.getElementById('poison-index');
  const reveal = ['#band', '#verdict'].flatMap((sel) => document.querySelector(sel) ?? []);
  if (!fill || !knob || !readout) return;

  const colourAt = gsap.utils.interpolate(RISK_STOPS);
  const gauge = { value: 0 };
  const render = () => {
    fill.style.strokeDasharray = `${gauge.value} 100`;
    knob.style.stroke = colourAt(gauge.value / 100);
    readout.textContent = String(Math.round(gauge.value));
  };

  gsap.set(fill, { autoAlpha: value > 0 ? 1 : 0 });
  render(); // paint the zero state now, so the placeholder never shows
  gsap.set(knob, { rotation: 0, svgOrigin: '110 110' });

  // matchMedia only runs the handler when a condition matches, so list both.
  const mm = gsap.matchMedia();
  mm.add(
    {
      motion: '(prefers-reduced-motion: no-preference)',
      reduce: '(prefers-reduced-motion: reduce)',
    },
    (context) => {
      const tl = gsap.timeline({
        defaults: { duration: 0.6, ease: 'power3.out' },
        onComplete: () => {
          readout.textContent = formatIndex(index);
        },
      });

      tl.addLabel('sweep', 0)
        .to(gauge, { value, onUpdate: render }, 'sweep')
        .to(knob, { rotation: value * 1.8, svgOrigin: '110 110' }, 'sweep')
        .from(reveal, { autoAlpha: 0, y: 6, duration: 0.3, stagger: 0.06 }, 'sweep+=0.25');

      if (context.conditions?.reduce) tl.progress(1);
    },
  );
}

async function load(): Promise<void> {
  const stored = (await chrome.storage.local.get('last_scan')) as {
    last_scan?: ScanRecord;
  };
  const record = stored.last_scan;

  if (!record) {
    show('none');
    return;
  }

  const { report } = record;
  const band = bandFor(report.poison_index);
  const summary = document.getElementById('summary');
  summary?.setAttribute('data-band', band.band);

  setText('band', band.label);
  setText('verdict', band.verdict);

  const value = Math.max(0, Math.min(100, report.poison_index));
  animateScore(value, report.poison_index);
  summary?.querySelector('.gauge')?.setAttribute('aria-valuenow', String(value));

  const host = document.getElementById('source-host');
  if (host) {
    host.textContent = hostOf(record.url);
    host.title = record.url;
  }
  const time = document.querySelector<HTMLTimeElement>('#scanned-at');
  if (time) {
    time.textContent = relativeTime(record.scanned_at);
    time.dateTime = record.scanned_at;
    time.title = new Date(record.scanned_at).toLocaleString();
  }
  show('summary');

  if (report.truncated) show('truncated');

  if (report.findings.length === 0) {
    show('empty');
    return;
  }

  const container = document.getElementById('report');
  container?.append(...renderGroups(report.findings));
  container?.querySelectorAll<HTMLElement>('blockquote.clamped').forEach(makeExpandable);
}

void load();
