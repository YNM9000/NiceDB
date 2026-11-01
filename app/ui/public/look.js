try {
  const l = JSON.parse(localStorage.getItem('nd-look') || 'null');
  const r = document.documentElement;
  if (l) {
    r.dataset.style = l.s;
    r.dataset.palette = l.p;
    r.dataset.theme = l.t;
    if (l.bg) r.style.background = l.bg;
  }
} catch {}
