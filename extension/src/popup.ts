// Popup: a plain reference view of the last stored report.
//
// No highlighting, jump-to-clause, mail generation or history. Those are product
// features layered on top of the contract in docs/CONTRACT.md.

import type { ClauseFinding, ScanRecord } from './types';

const empty = document.getElementById('empty');
const report = document.getElementById('report');
const index = document.getElementById('poison-index');

function renderFinding(finding: ClauseFinding): HTMLLIElement {
  const item = document.createElement('li');

  const heading = document.createElement('p');
  heading.textContent =
    `${finding.label} — ${finding.severity.toFixed(1)}/5.0 ` +
    `(${finding.rule_id})`;

  const quote = document.createElement('blockquote');
  quote.textContent = finding.raw_text;

  const meta = document.createElement('p');
  meta.textContent = finding.opt_out_available
    ? finding.opt_out_target
      ? `Opt-out available: ${finding.opt_out_target}`
      : 'Opt-out available'
    : 'No stated opt-out';

  item.append(heading, quote, meta);
  return item;
}

async function load(): Promise<void> {
  const stored = (await chrome.storage.local.get('last_scan')) as {
    last_scan?: ScanRecord;
  };
  const record = stored.last_scan;

  if (!record) {
    document.getElementById('none')?.removeAttribute('hidden');
    return;
  }

  // The record is the most recent scan of any page, so say which one.
  const sourceUrl = document.getElementById('source-url');
  if (sourceUrl) sourceUrl.textContent = record.url;
  document.getElementById('source')?.removeAttribute('hidden');

  if (index) {
    index.textContent = record.report.poison_index.toFixed(1);
  }

  if (record.report.findings.length === 0) {
    empty?.removeAttribute('hidden');
    return;
  }

  const list = document.createElement('ul');
  for (const finding of record.report.findings) {
    list.appendChild(renderFinding(finding));
  }
  report?.append(list);
}

void load();
