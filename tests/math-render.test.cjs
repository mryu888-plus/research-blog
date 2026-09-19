const { test } = require('node:test');
const assert = require('node:assert/strict');
const { renderArticleMath } = require('../site/static/main.js');

function formula(tex, display = false) {
  return {
    dataset: { mathTex: tex, mathDisplay: String(display) },
    textContent: display ? `$$\n${tex}\n$$` : `$${tex}$`,
  };
}

function article(nodes) {
  return { querySelectorAll: () => nodes };
}

test('protected TeX reaches KaTeX unchanged with the correct inline and display modes', () => {
  const tex = String.raw`\begin{aligned}
J_{\theta} &= \{x_i\}_{i=1}^{n} \\
&= a_b * c_d < e
\end{aligned}`;
  const block = formula(tex, true);
  const inline = formula(String.raw`x_{i} + \alpha`);
  const calls = [];
  const renderer = { render: (source, node, options) => calls.push({ source, node, options }) };
  renderArticleMath(article([block, inline]), renderer, null);
  assert.equal(calls[0].source, tex);
  assert.equal(calls[0].node, block);
  assert.equal(calls[0].options.displayMode, true);
  assert.equal(calls[1].source, inline.dataset.mathTex);
  assert.equal(calls[1].options.displayMode, false);
  assert.equal(calls.every(call => call.options.throwOnError === false), true);
});

test('repeated initialization skips rendered formulas and renders newly replaced preview content', () => {
  const first = formula('x^2');
  const nodes = [first];
  let renders = 0;
  const renderer = { render: (_, node) => { renders++; node.textContent = 'typeset formula'; } };
  const content = article(nodes);
  renderArticleMath(content, renderer, null);
  renderArticleMath(content, renderer, null);
  assert.equal(renders, 1);
  assert.equal(first.textContent, 'typeset formula');
  nodes.splice(0, 1, formula('y^3'));
  renderArticleMath(content, renderer, null);
  assert.equal(renders, 2);
});

test('missing KaTeX leaves the original formula readable and can render when the library arrives', () => {
  const node = formula(String.raw`\frac{a_1}{b_2}`, true);
  const fallback = node.textContent;
  const content = article([node]);
  renderArticleMath(content, null, null);
  assert.equal(node.textContent, fallback);
  assert.equal(node.dataset.mathRendered, undefined);
  let rendered = false;
  renderArticleMath(content, { render: () => { rendered = true; } }, null);
  assert.equal(rendered, true);
  assert.equal(node.dataset.mathRendered, 'true');
});

test('unexpected renderer errors restore text and do not prevent other formulas from rendering', () => {
  const bad = formula(String.raw`\broken{x}`);
  const good = formula('x+1');
  const fallback = bad.textContent;
  renderArticleMath(article([bad, good]), { render: (_, node) => {
    if (node === bad) { node.textContent = ''; throw new Error('renderer failure'); }
    node.textContent = 'x + 1';
  } }, null);
  assert.equal(bad.textContent, fallback);
  assert.equal(bad.dataset.mathRendered, undefined);
  assert.equal(good.dataset.mathRendered, 'true');
});

test('legacy auto-render runs after protected formulas and excludes protected, error, and rendered math', () => {
  const node = formula(String.raw`\text{a $ b}`);
  const content = article([node]);
  let legacyCalls = 0;
  renderArticleMath(content, { render: (_, element) => { element.textContent = 'KaTeX error output'; } }, (root, options) => {
    legacyCalls++;
    assert.equal(root, content);
    assert.equal(node.dataset.mathRendered, 'true');
    assert.equal(node.textContent, 'KaTeX error output');
    assert.ok(options.ignoredClasses.includes('math-source'));
    assert.ok(options.ignoredClasses.includes('katex'));
    assert.deepEqual(options.delimiters.slice(0, 2), [
      { left: '$$', right: '$$', display: true },
      { left: '$', right: '$', display: false },
    ]);
    assert.ok(options.delimiters.some(delimiter => delimiter.left === '\\(' && !delimiter.display));
    assert.ok(options.delimiters.some(delimiter => delimiter.left === '\\[' && delimiter.display));
    assert.equal(options.throwOnError, false);
  });
  assert.equal(legacyCalls, 1);
});

test('legacy renderer also ignores protected source while KaTeX is unavailable', () => {
  const node = formula('a_b * c_d');
  const fallback = node.textContent;
  renderArticleMath(article([node]), null, (_, options) => {
    assert.ok(options.ignoredClasses.includes('math-source'));
  });
  assert.equal(node.textContent, fallback);
  assert.doesNotThrow(() => renderArticleMath(null, null, null));
});

test('prepared pages never reinterpret escaped dollars and prices as legacy math', () => {
  const node = formula('x_1');
  const content = { ...article([node]), dataset: { mathPrepared: 'true' } };
  let renders = 0;
  renderArticleMath(content, { render: () => { renders++; } }, () => {
    assert.fail('Prepared text must not pass through delimiter detection again');
  });
  assert.equal(renders, 1);
  renderArticleMath({ ...article([]), dataset: { mathPrepared: 'true' } }, null, () => {
    assert.fail('Escaped dollars must stay literal even on pages with no formulas');
  });
});
