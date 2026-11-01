const escape = (s: string) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;').replace(/'/g, '&#39;');

const SAFE_URL = /^(https?:\/\/|mailto:)[^\s<>"']*$/i;

function inline(raw: string): string {
  const codes: string[] = [];
  let s = escape(raw).replace(/`([^`\n]+)`/g, (_, c: string) => `\u0000${codes.push(`<code>${c}</code>`) - 1}\u0000`);
  s = s.replace(/\[([^\]\n]+)\]\(([^)\s]+)\)/g, (whole, text: string, url: string) => {
    const plain = url.replace(/&amp;/g, '&');
    return SAFE_URL.test(plain) ? `<a href="${escape(plain)}" rel="noopener noreferrer">${text}</a>` : whole;
  });
  s = s.replace(/\*\*([^*\n]+)\*\*/g, '<strong>$1</strong>').replace(/(^|[^*])\*([^*\n]+)\*(?!\*)/g, '$1<em>$2</em>').replace(/~~([^~\n]+)~~/g, '<del>$1</del>');
  return s.replace(/\u0000(\d+)\u0000/g, (_, i: string) => codes[Number(i)] ?? '');
}

type Block = { kind: 'p' | 'ul' | 'ol' | 'quote'; lines: string[] };

export function renderMarkdown(src: string): string {
  const lines = src.replace(/\r\n?/g, '\n').split('\n');
  const out: string[] = [];
  let block: Block | null = null;
  const flush = () => {
    if (!block) return;
    if (block.kind === 'p') out.push(`<p>${block.lines.map(inline).join('<br>')}</p>`);
    else if (block.kind === 'quote') out.push(`<blockquote>${block.lines.map(inline).join('<br>')}</blockquote>`);
    else {
      const items = block.lines.map((l) => {
        const task = /^\[([ xX])\]\s+(.*)$/.exec(l);
        if (task) return `<li class="task"><input type="checkbox" disabled${task[1] === ' ' ? '' : ' checked'}> ${inline(task[2] ?? '')}</li>`;
        return `<li>${inline(l)}</li>`;
      });
      out.push(`<${block.kind}>${items.join('')}</${block.kind}>`);
    }
    block = null;
  };
  const add = (kind: Block['kind'], line: string) => {
    if (block?.kind !== kind) {
      flush();
      block = { kind, lines: [] };
    }
    block.lines.push(line);
  };

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i] ?? '';
    const fence = /^```\s*(\w*)\s*$/.exec(line);
    if (fence) {
      flush();
      const code: string[] = [];
      for (i++; i < lines.length && !/^```\s*$/.test(lines[i] ?? ''); i++) code.push(lines[i] ?? '');
      out.push(`<pre><code>${escape(code.join('\n'))}</code></pre>`);
      continue;
    }
    const head = /^(#{1,4})\s+(.*)$/.exec(line);
    const bullet = /^\s*[-*+]\s+(.*)$/.exec(line);
    const number = /^\s*\d+[.)]\s+(.*)$/.exec(line);
    const quote = /^>\s?(.*)$/.exec(line);
    if (head) {
      flush();
      const level = (head[1] ?? '#').length;
      out.push(`<h${level}>${inline(head[2] ?? '')}</h${level}>`);
    } else if (/^\s*([-*_])(\s*\1){2,}\s*$/.test(line)) {
      flush();
      out.push('<hr>');
    } else if (bullet) add('ul', bullet[1] ?? '');
    else if (number) add('ol', number[1] ?? '');
    else if (quote) add('quote', quote[1] ?? '');
    else if (line.trim() === '') flush();
    else add('p', line);
  }
  flush();
  return out.join('\n');
}
