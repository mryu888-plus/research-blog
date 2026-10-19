'use strict';
const samples = {
  editorial: { number:'01',title:'语义场',file:'semantic-field.png',description:'以连续的等高线表达语义空间，以一个红色焦点表达局部邻域。图像先建立直觉，再由正文补足解释。',use:'文章开篇、章节间的主题配图',layout:'一个主体 · 疏密对照 · 适量留白' },
  concept: { number:'02',title:'不同形式，同一关系',file:'equivalence-plate.png',description:'三种轮廓，共享一条轴线。通过共同的几何约束表达联系，颜色落在真正需要注意的关系上。',use:'概念比较、关系说明、关键不变量',layout:'至多三个主体 · 共同轴线 · 明确联系' },
  cover: { number:'03',title:'知识层叠',file:'knowledge-strata-v2.png',description:'用重复切片和一处结构变化表现知识的积累与重组。主体偏向右侧，留出可由网页排版的标题区域。',use:'系列封面、长文首图、主题导读',layout:'一个强轮廓 · 右侧主体 · 左侧留白' },
};
const buttons = [...document.querySelectorAll('[role=tab]')];
function select(mode) {
  const sample=samples[mode];
  for(const button of buttons){const active=button.dataset.mode===mode;button.setAttribute('aria-selected',String(active));button.tabIndex=active?0:-1;}
  document.getElementById('plate-panel').setAttribute('aria-labelledby','tab-'+mode);
  const image=document.getElementById('gallery-image');image.src='assets/'+sample.file;image.alt=sample.description;
  for(const [id,value] of Object.entries({'gallery-number':'PLATE '+sample.number,'gallery-title':sample.title,'gallery-description':sample.description,'gallery-use':sample.use,'gallery-layout':sample.layout,'gallery-setting':'早期探索 / '+mode}))document.getElementById(id).textContent=value;
  document.getElementById('gallery-download').href=image.getAttribute('src');
}
for(const button of buttons){button.addEventListener('click',()=>select(button.dataset.mode));button.addEventListener('keydown',event=>{const i=buttons.indexOf(button);let next;if(event.key==='ArrowRight')next=buttons[(i+1)%buttons.length];if(event.key==='ArrowLeft')next=buttons[(i+buttons.length-1)%buttons.length];if(event.key==='Home')next=buttons[0];if(event.key==='End')next=buttons.at(-1);if(next){event.preventDefault();select(next.dataset.mode);next.focus();}});}
document.getElementById('theme').addEventListener('click',()=>{const light=document.documentElement.dataset.theme!=='light';document.documentElement.dataset.theme=light?'light':'dark';const button=document.getElementById('theme');button.querySelector('span').textContent=light?'石墨黑':'暖纸白';button.setAttribute('aria-label',light?'切换深色预览':'切换浅色预览');});
// Swatch labels follow the shared CSS too, so a ZEN edit cannot leave stale hexes.
for (const swatch of document.querySelectorAll('.swatches > div')) {
  const variable = swatch.style.getPropertyValue('--swatch').match(/var\((--[\w-]+)\)/)?.[1];
  if (variable) swatch.querySelector('code').textContent = getComputedStyle(document.documentElement).getPropertyValue(variable).trim().toUpperCase();
}
