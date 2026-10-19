(() => {
  const input = document.querySelector('[data-reference-search]');
  const status = document.querySelector('[data-reference-status]');
  if (!input || !status) return;
  const rows = [...document.querySelectorAll('.reference-library > li')];
  const terms = rows.map(row => row.textContent.normalize('NFKC').toLocaleLowerCase());
  function update() {
    const query = input.value.normalize('NFKC').trim().toLocaleLowerCase().split(/\s+/).filter(Boolean);
    let visible = 0;
    rows.forEach((row, index) => {
      row.hidden = !query.every(word => terms[index].includes(word));
      if (!row.hidden) visible += 1;
    });
    status.textContent = query.length ? `找到 ${visible} / ${rows.length} 篇文献` : `共 ${rows.length} 篇文献`;
  }
  input.addEventListener('input', update);
  update();
})();
