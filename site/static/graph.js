/* Deterministic SVG graph. Motion runs only on entry and direct interaction. */
(() => {
  'use strict';
  const SVG = 'http://www.w3.org/2000/svg';
  const layouts = new Map();
  const instances = new WeakMap();

  function safeUrl(value, base = 'https://example.invalid/') {
    try {
      const url = new URL(String(value), base);
      return ['http:', 'https:'].includes(url.protocol) ? url.href : null;
    } catch (_) { return null; }
  }

  function normalizeGraph(data, base) {
    const nodes = [], ids = new Set();
    for (const raw of data?.nodes || []) {
      if (raw?.id == null || !raw.url) continue;
      const id = String(raw.id), url = safeUrl(raw.url, base);
      if (!id || ids.has(id) || !url) continue;
      ids.add(id);
      nodes.push({ id, url, title:String(raw.title || id), tags:Array.isArray(raw.tags) ? raw.tags.map(String) : [], draft:raw.draft === true });
    }
    nodes.sort((a,b) => a.id < b.id ? -1 : a.id > b.id ? 1 : 0);
    const edges = [], seen = new Set();
    for (const raw of data?.edges || []) {
      const source = String(raw?.source ?? ''), target = String(raw?.target ?? '');
      const key = JSON.stringify([source,target]);
      if (source === target || !ids.has(source) || !ids.has(target) || seen.has(key)) continue;
      seen.add(key); edges.push({source,target});
    }
    edges.sort((a,b) => a.source < b.source ? -1 : a.source > b.source ? 1 : a.target < b.target ? -1 : a.target > b.target ? 1 : 0);
    return {nodes,edges};
  }

  function neighborhood(graph, id) {
    const ids = new Set([id]);
    for (const edge of graph.edges) {
      if (edge.source === id) ids.add(edge.target);
      if (edge.target === id) ids.add(edge.source);
    }
    return ids;
  }

  function filterGraph(graph, query = '', focusId = '') {
    const nearby = focusId ? neighborhood(graph,focusId) : null;
    const term = query.trim().toLocaleLowerCase();
    const nodes = graph.nodes.filter(node => (!nearby || nearby.has(node.id)) &&
      (!term || `${node.title} ${node.tags.join(' ')}`.toLocaleLowerCase().includes(term)));
    const ids = new Set(nodes.map(node => node.id));
    return {nodes,edges:graph.edges.filter(edge => ids.has(edge.source) && ids.has(edge.target))};
  }

  function layoutGraph(graph) {
    const key = JSON.stringify([graph.nodes.map(node => node.id),graph.edges]);
    if (layouts.has(key)) return new Map(layouts.get(key).map(([id,p]) => [id,{...p}]));
    const positions = new Map();
    graph.nodes.forEach((node,index) => {
      const radius = graph.nodes.length === 1 ? 0 : 35 * Math.sqrt(index + 1);
      const angle = index * 2.3999632297;
      positions.set(node.id,{x:radius*Math.cos(angle),y:radius*Math.sin(angle)});
    });
    // Bound all-pairs work. Large collections retain a stable sunflower layout.
    if (graph.nodes.length <= 180) {
      const points = Array.from(positions.values());
      const byId = new Map(graph.nodes.map((node,index) => [node.id,index]));
      for (let step=0; step<60; step++) {
        const forces = points.map(point => ({x:-point.x*.003,y:-point.y*.003}));
        for (let i=0; i<points.length; i++) for (let j=i+1; j<points.length; j++) {
          const dx=points[i].x-points[j].x, dy=points[i].y-points[j].y;
          const distance=Math.max(16,Math.hypot(dx,dy)), force=1200/(distance*distance);
          forces[i].x += dx/distance*force; forces[i].y += dy/distance*force;
          forces[j].x -= dx/distance*force; forces[j].y -= dy/distance*force;
        }
        for (const edge of graph.edges) {
          const i=byId.get(edge.source), j=byId.get(edge.target);
          const dx=points[j].x-points[i].x, dy=points[j].y-points[i].y;
          const distance=Math.max(1,Math.hypot(dx,dy)), force=(distance-115)*.012;
          forces[i].x += dx/distance*force; forces[i].y += dy/distance*force;
          forces[j].x -= dx/distance*force; forces[j].y -= dy/distance*force;
        }
        points.forEach((point,index) => {
          point.x += Math.max(-8,Math.min(8,forces[index].x));
          point.y += Math.max(-8,Math.min(8,forces[index].y));
        });
      }
    }
    layouts.set(key,Array.from(positions,([id,p]) => [id,{...p}]));
    if (layouts.size > 8) layouts.delete(layouts.keys().next().value);
    return positions;
  }

  function element(tag, className, text) {
    const node=document.createElement(tag);
    if (className) node.className=className;
    if (text != null) node.textContent=text;
    return node;
  }
  function svgElement(tag, attrs={}) {
    const node=document.createElementNS(SVG,tag);
    for (const [name,value] of Object.entries(attrs)) node.setAttribute(name,String(value));
    return node;
  }

  // At most one finite camera transition exists. Dragging, rebuilding, reduced
  // motion changes, and disposal all interrupt it immediately.
  function createCameraMotion({request, cancel, now, reduced, apply}) {
    let frame=null, target=null;
    function stop() { if (frame!==null) cancel(frame); frame=null; target=null; }
    function finish() { const destination=target; stop(); if (destination) apply(destination); }
    function move(from, to, duration=380) {
      stop();
      if (reduced() || duration<=0) { apply(to); return; }
      target=to;
      const started=now();
      function tick(timestamp) {
        const progress=Math.max(0,Math.min(1,(timestamp-started)/duration));
        if (progress===1) { apply(to); frame=null; target=null; return; }
        const eased=1-Math.pow(1-progress,3);
        apply({zoom:from.zoom+(to.zoom-from.zoom)*eased,pan:{
          x:from.pan.x+(to.pan.x-from.pan.x)*eased,
          y:from.pan.y+(to.pan.y-from.pan.y)*eased,
        }});
        frame=request(tick);
      }
      frame=request(tick);
    }
    return {move,stop,finish};
  }

  function render(container, data, options={}) {
    instances.get(container)?.();
    let WIDTH=960, HEIGHT=560;
    const graph=normalizeGraph(data,location.href), positions=layoutGraph(graph);
    const byId=new Map(graph.nodes.map(node => [node.id,node]));
    const saved=options.state;
    if (Array.isArray(saved?.positions)) {
      for (const entry of saved.positions) {
        if (!Array.isArray(entry)) continue;
        const [id,point]=entry;
        if (positions.has(id) && Number.isFinite(point?.x) && Number.isFinite(point?.y)) positions.set(id,{x:point.x,y:point.y});
      }
    }
    const abort=new AbortController(), on=(node,type,fn,settings={}) => node.addEventListener(type,fn,{...settings,signal:abort.signal});
    const motionPreference=window.matchMedia('(prefers-reduced-motion: reduce)');
    const preferredFocus=saved?.focusId ?? options.focusId;
    let focusId=byId.has(preferredFocus) ? preferredFocus : '';
    let selectedId=byId.has(saved?.selectedId) ? saved.selectedId : '';
    let visible, zoom=1, pan={x:WIDTH/2,y:HEIGHT/2}, gesture=null, suppressClick=false, hoveredId='';
    let nodeElements=new Map(), edgeElements=[];
    const adjacent=new Map(graph.nodes.map(node => [node.id,new Set()]));
    graph.edges.forEach(edge => { adjacent.get(edge.source).add(edge.target); adjacent.get(edge.target).add(edge.source); });
    container.classList.add('knowledge-graph');
    container.classList.toggle('is-reduced-motion',motionPreference.matches);
    container.replaceChildren();

    function button(parent,label,content,fn,className='') {
      const control=element('button',className,typeof content==='string'?content:null);
      control.type='button'; control.setAttribute('aria-label',label); control.title=label;
      if (typeof content!=='string') control.append(content);
      on(control,'click',fn); parent.append(control); return control;
    }
    function icon(path) {
      const svg=svgElement('svg',{viewBox:'0 0 24 24','aria-hidden':'true'});
      svg.append(svgElement('path',{d:path})); return svg;
    }
    const toolbar=element('div','graph-toolbar');
    const searchLabel=element('label','graph-search');
    searchLabel.append(icon('M16.5 16.5 21 21 M18 10.5a7.5 7.5 0 1 1-15 0 7.5 7.5 0 0 1 15 0'));
    const search=element('input'); search.type='search'; search.placeholder='搜索文章或主题'; search.autocomplete='off';
    search.setAttribute('aria-label','查找文章或主题');
    if (typeof saved?.query==='string') search.value=saved.query;
    searchLabel.append(search);
    const status=element('p','graph-status'); status.setAttribute('role','status'); status.setAttribute('aria-live','polite');
    toolbar.append(searchLabel,status);

    const canvas=element('div','graph-canvas');
    const svg=svgElement('svg',{class:'graph-svg',viewBox:`0 0 ${WIDTH} ${HEIGHT}`,role:'group',tabindex:0,
      'aria-label':'文章关联图。选择节点查看详情；方向键移动，加减键缩放。全部文章也可在下方索引中阅读。'});
    const layer=svgElement('g',{class:'graph-layer'}); svg.append(layer); canvas.append(svg);
    const zoomControls=element('div','graph-zoom'); zoomControls.setAttribute('role','group'); zoomControls.setAttribute('aria-label','图谱视图');
    button(zoomControls,'缩小图谱',icon('M6 12h12'),() => scale(1/1.25));
    button(zoomControls,'放大图谱',icon('M6 12h12 M12 6v12'),() => scale(1.25));
    button(zoomControls,'重置筛选与视图',icon('M8 4H4v4 M4 4l5 5 M16 4h4v4 M20 4l-5 5 M4 16v4h4 M4 20l5-5 M20 16v4h-4 M20 20l-5-5'),reset);
    canvas.append(zoomControls);

    const selection=element('aside','graph-selection'); selection.hidden=true;
    selection.setAttribute('aria-label','所选文章');
    const selectionText=element('div','graph-selection-text');
    const selectionTitle=element('a','graph-selection-title');
    const selectionMeta=element('p','graph-selection-meta');
    selectionText.append(selectionTitle,selectionMeta);
    const selectionActions=element('div','graph-selection-actions');
    const readLink=element('a','graph-read-link','阅读'); readLink.append(icon('M5 12h14 M13 6l6 6-6 6')); selectionActions.append(readLink);
    button(selectionActions,'查看所选文章的关联','关联',() => setFocus(selectedId),'graph-related');
    button(selection,'关闭文章详情',icon('M6 6l12 12 M6 18 18 6'),() => selectNode('',false,true),'graph-selection-close');
    selection.prepend(selectionText,selectionActions); canvas.append(selection);

    const articles=element('details','graph-articles'); articles.open=saved?.panelOpen===true;
    const summary=element('summary'); summary.append(element('span','','文章列表'),icon('M8 10l4 4 4-4'));
    const panel=element('div','graph-panel');
    const focusLabel=element('label','graph-field'); focusLabel.append(element('span','','查看文章关联'));
    const focus=element('select'); focus.setAttribute('aria-label','选择一篇文章查看关联'); focus.append(new Option('全部文章',''));
    graph.nodes.forEach(node => focus.append(new Option(node.title,node.id)));
    focus.value=focusId; focusLabel.append(focus);
    const help=element('p','graph-help','点击文章节点查看详情，拖动或缩放图谱查看其他文章。也可以用方向键移动，＋ / − 缩放，Esc 收起详情。');
    const list=element('ul','graph-article-list'); panel.append(focusLabel,help,list); articles.append(summary,panel);
    container.append(toolbar,canvas,articles);
    const stage=svg.getBoundingClientRect();
    WIDTH=stage.width || WIDTH; HEIGHT=stage.height || HEIGHT;
    svg.setAttribute('viewBox',`0 0 ${WIDTH} ${HEIGHT}`);

    function applyView() { layer.setAttribute('transform',`translate(${pan.x} ${pan.y}) scale(${zoom})`); }
    const camera=createCameraMotion({
      request:callback => window.requestAnimationFrame(callback), cancel:id => window.cancelAnimationFrame(id),
      now:() => performance.now(), reduced:() => motionPreference.matches,
      apply:view => {zoom=view.zoom;pan={...view.pan};applyView();},
    });
    function moveView(target,animated=true) { camera.move({zoom,pan:{...pan}},target,animated?380:0); }
    function scale(factor,center={x:WIDTH/2,y:HEIGHT/2},animated=true) {
      const next=Math.max(.3,Math.min(3,zoom*factor)), ratio=next/zoom;
      moveView({zoom:next,pan:{x:center.x-(center.x-pan.x)*ratio,y:center.y-(center.y-pan.y)*ratio}},animated);
    }
    function fit(animated=true) {
      const points=visible.nodes.map(node => positions.get(node.id));
      if (!points.length) {moveView({zoom:1,pan:{x:WIDTH/2,y:HEIGHT/2}},animated);return;}
      const xs=points.map(point => point.x), ys=points.map(point => point.y);
      const minX=Math.min(...xs),maxX=Math.max(...xs),minY=Math.min(...ys),maxY=Math.max(...ys);
      const next=Math.max(.15,Math.min(1.3,(WIDTH-150)/Math.max(180,maxX-minX),(HEIGHT-160)/Math.max(180,maxY-minY)));
      moveView({zoom:next,pan:{x:WIDTH/2-(minX+maxX)/2*next,y:HEIGHT*.46-(minY+maxY)/2*next}},animated);
    }
    function highlight(id) {
      const nearby=id && nodeElements.has(id)?neighborhood(graph,id):null;
      layer.classList.toggle('has-highlight',!!nearby);
      nodeElements.forEach((node,nodeId) => {
        node.classList.toggle('is-active',nodeId===id);
        node.classList.toggle('is-selected',nodeId===selectedId);
        node.classList.toggle('is-muted',!!nearby && !nearby.has(nodeId));
      });
      edgeElements.forEach(({line,edge}) => {
        const active=edge.source===id || edge.target===id;
        line.classList.toggle('is-active',active);
        line.classList.toggle('is-muted',!!nearby && !active);
      });
    }
    function updateSelection() {
      const node=byId.get(selectedId);
      selection.hidden=!node;
      if (node) {
        selectionTitle.textContent=node.title; selectionTitle.href=node.url; readLink.href=node.url;
        readLink.setAttribute('aria-label',`阅读「${node.title}」`);
        const count=adjacent.get(node.id).size;
        selectionMeta.textContent=count?`与 ${count} 篇文章有关联`:'这篇笔记暂时没有关联文章。';
      }
      highlight(hoveredId || selectedId || focusId);
    }
    function selectNode(id,animate=true,restoreFocus=false) {
      const previous=selectedId;
      selectedId=byId.has(id) && nodeElements.has(id)?id:'';
      updateSelection();
      if (selectedId && animate) {
        const point=positions.get(selectedId);
        moveView({zoom,pan:{x:WIDTH/2-point.x*zoom,y:HEIGHT*.4-point.y*zoom}});
      }
      if (restoreFocus) nodeElements.get(previous)?.focus({preventScroll:true});
    }
    function setFocus(id) {
      focusId=byId.has(id)?id:''; focus.value=focusId; search.value=''; redraw();
    }
    function reset() { search.value=''; focus.value=''; focusId=''; selectedId=''; hoveredId=''; redraw(); }
    function updatePositions() {
      nodeElements.forEach((node,id) => {const p=positions.get(id);node.setAttribute('transform',`translate(${p.x} ${p.y})`);});
      edgeElements.forEach(({line,edge}) => {
        const from=positions.get(edge.source),to=positions.get(edge.target),dx=to.x-from.x,dy=to.y-from.y;
        const distance=Math.max(1,Math.hypot(dx,dy)),padding=13;
        line.setAttribute('x1',from.x+dx/distance*padding); line.setAttribute('y1',from.y+dy/distance*padding);
        line.setAttribute('x2',to.x-dx/distance*padding); line.setAttribute('y2',to.y-dy/distance*padding);
      });
    }
    function redraw(animated=true) {
      visible=filterGraph(graph,search.value,focusId); nodeElements=new Map(); edgeElements=[]; hoveredId='';
      layer.replaceChildren(); list.replaceChildren(); canvas.querySelector('.graph-empty')?.remove();
      for (const [index,edge] of visible.edges.entries()) {
        const line=svgElement('line',{class:'graph-edge',pathLength:1});
        line.style.setProperty('--enter-delay',`${Math.min(index*24,240)}ms`);
        layer.append(line); edgeElements.push({line,edge});
      }
      for (const [index,node] of visible.nodes.entries()) {
        const count=adjacent.get(node.id).size;
        const link=svgElement('a',{href:node.url,class:'graph-node','data-node':node.id,
          'aria-label':`${node.title}，${count} 篇关联文章，选择查看详情`,tabindex:0});
        link.style.setProperty('--enter-delay',`${Math.min(index*45,360)}ms`);
        const title=svgElement('title'); title.textContent=node.title;
        const text=svgElement('text',{y:30}); const chars=Array.from(node.title);
        text.textContent=chars.length>16?chars.slice(0,15).join('')+'…':node.title;
        const hitWidth=Math.max(48,Array.from(text.textContent).reduce((width,char) => width+(char.codePointAt(0)>255?13:7),0)+14);
        const hit=svgElement('rect',{x:-hitWidth/2,y:-20,width:hitWidth,height:58,rx:5,fill:'transparent','pointer-events':'all','aria-hidden':'true'});
        const mark=svgElement('g',{class:'graph-node-mark','aria-hidden':'true'});
        mark.append(svgElement('circle',{class:'graph-node-halo',r:22}),svgElement('circle',{class:'graph-node-dot',r:5+Math.min(5,Math.sqrt(count)*1.3)}));
        link.append(title,hit,mark,text); layer.append(link); nodeElements.set(node.id,link);
        const item=element('li'), articleLink=element('a','',node.title); articleLink.href=node.url;
        if (node.draft) articleLink.append(element('span','graph-draft','草稿'));
        const countLabel=element('span','graph-article-count',String(count)); countLabel.title=`${count} 篇关联文章`;
        const focusButton=element('button','','关联'); focusButton.type='button'; focusButton.dataset.focus=node.id;
        focusButton.setAttribute('aria-label',`只看「${node.title}」的关联`);
        item.append(articleLink,countLabel,focusButton); list.append(item);
      }
      if (!nodeElements.has(selectedId)) selectedId='';
      updatePositions(); fit(animated); updateSelection();
      status.textContent=`${visible.nodes.length} 篇文章${visible.edges.length?` · ${visible.edges.length} 条引用`:''}`;
      if (!visible.nodes.length) {
        const empty=element('div','graph-empty');
        empty.append(element('strong','',graph.nodes.length?'没有找到相关文章':'还没有发布的笔记'),element('p','',graph.nodes.length?'换个关键词试试。':'发布第一篇笔记后，就能在这里看到它。'));
        const restore=button(empty,'清除图谱筛选','清除筛选',reset); restore.hidden=!graph.nodes.length;
        canvas.append(empty);
      }
    }
    const localPoint=event => {
      const point=new DOMPoint(event.clientX,event.clientY);
      const matrix=svg.getScreenCTM(); return matrix?point.matrixTransform(matrix.inverse()):point;
    };
    on(motionPreference,'change',() => {
      container.classList.toggle('is-reduced-motion',motionPreference.matches);
      if (motionPreference.matches) camera.finish();
    });
    on(search,'input',() => redraw());
    on(focus,'change',() => {focusId=focus.value; redraw();});
    on(list,'click',event => {
      const control=event.target.closest('button[data-focus]'); if (!control) return;
      setFocus(control.dataset.focus); focus.focus();
    });
    on(svg,'pointerover',event => {
      if (event.pointerType==='touch' || gesture?.moved) return;
      const id=event.target.closest('[data-node]')?.dataset.node;
      if (id && hoveredId!==id) {hoveredId=id;highlight(id);}
    });
    on(svg,'pointerout',event => {
      if (event.relatedTarget?.closest?.('[data-node]')?.dataset.node===hoveredId) return;
      hoveredId=''; highlight(selectedId || focusId);
    });
    on(svg,'focusin',event => {const id=event.target.closest('[data-node]')?.dataset.node;if(id)highlight(id);});
    on(svg,'focusout',event => {if(!event.relatedTarget?.closest?.('[data-node]'))highlight(selectedId || focusId);});
    on(svg,'wheel',event => {event.preventDefault();scale(Math.exp(-Math.max(-120,Math.min(120,event.deltaY))*.002),localPoint(event),false);},{passive:false});
    on(container,'keydown',event => {
      if (event.key==='Escape' && selectedId) {event.preventDefault();selectNode('',false,true);return;}
      if (event.target!==svg) return;
      const directions={ArrowLeft:[40,0],ArrowRight:[-40,0],ArrowUp:[0,40],ArrowDown:[0,-40]};
      if (directions[event.key]) {event.preventDefault();moveView({zoom,pan:{x:pan.x+directions[event.key][0],y:pan.y+directions[event.key][1]}});}
      else if (['+','=','-'].includes(event.key)) {event.preventDefault();scale(event.key==='-'?1/1.25:1.25);}
    });
    on(svg,'pointerdown',event => {
      if (event.button!==0) return;
      camera.stop();
      const point=localPoint(event), id=event.target.closest('[data-node]')?.dataset.node;
      gesture={pointerId:event.pointerId,start:point,pan:{...pan},id,position:id?{...positions.get(id)}:null,moved:false};
      suppressClick=false;
    });
    on(svg,'pointermove',event => {
      if (!gesture || gesture.pointerId!==event.pointerId) return;
      const point=localPoint(event), dx=point.x-gesture.start.x,dy=point.y-gesture.start.y;
      if (!gesture.moved && Math.hypot(dx,dy)<4) return;
      if (!gesture.moved) {svg.setPointerCapture(event.pointerId);svg.classList.add('is-dragging');}
      gesture.moved=true;
      if (gesture.id) {positions.set(gesture.id,{x:gesture.position.x+dx/zoom,y:gesture.position.y+dy/zoom});updatePositions();}
      else {pan={x:gesture.pan.x+dx,y:gesture.pan.y+dy};applyView();}
    });
    function endGesture(event) {
      if (!gesture || gesture.pointerId!==event.pointerId) return;
      suppressClick=gesture.moved; gesture=null; svg.classList.remove('is-dragging');
      if (svg.hasPointerCapture(event.pointerId)) svg.releasePointerCapture(event.pointerId);
    }
    on(svg,'pointerup',endGesture);
    on(svg,'pointercancel',event => {endGesture(event);suppressClick=false;});
    on(svg,'lostpointercapture',endGesture);
    on(svg,'pointerleave',event => {if (gesture && !gesture.moved) endGesture(event);});
    on(svg,'click',event => {
      if (suppressClick) {event.preventDefault();event.stopPropagation();suppressClick=false;return;}
      const id=event.target.closest('[data-node]')?.dataset.node;
      if (id) {
        if (event.ctrlKey || event.metaKey || event.shiftKey || event.altKey) return;
        event.preventDefault(); selectNode(id);
      } else selectNode('',false);
    });
    redraw(false);
    if (Number.isFinite(saved?.zoom) && saved.zoom>0 && Number.isFinite(saved?.pan?.x) && Number.isFinite(saved?.pan?.y)) {
      zoom=Math.max(.15,Math.min(5,saved.zoom)); pan={...saved.pan}; applyView();
    }
    // Match SVG units to CSS pixels so labels remain readable on narrow screens.
    // A resize preserves the current viewpoint and never restarts entrance motion.
    const observer=typeof ResizeObserver==='undefined'?null:new ResizeObserver(() => {
      const bounds=svg.getBoundingClientRect();
      if (!bounds.width || !bounds.height || (bounds.width===WIDTH && bounds.height===HEIGHT)) return;
      camera.stop(); pan.x+=(bounds.width-WIDTH)/2; pan.y+=(bounds.height-HEIGHT)/2;
      WIDTH=bounds.width; HEIGHT=bounds.height; svg.setAttribute('viewBox',`0 0 ${WIDTH} ${HEIGHT}`); applyView();
    });
    observer?.observe(svg);
    let disposed=false;
    const cleanup=() => {
      if (disposed) return;
      disposed=true;
      camera.stop(); observer?.disconnect();
      if (gesture && svg.hasPointerCapture(gesture.pointerId)) svg.releasePointerCapture(gesture.pointerId);
      abort.abort();
      if (instances.get(container)===cleanup) instances.delete(container);
    };
    cleanup.state=() => ({focusId,selectedId,panelOpen:articles.open,query:search.value,zoom,pan:{...pan},positions:Array.from(positions,([id,point]) => [id,{...point}])});
    instances.set(container,cleanup); return cleanup;
  }
  if (typeof window !== 'undefined') window.BlogGraph={render};
  if (typeof module !== 'undefined' && module.exports) module.exports={safeUrl,normalizeGraph,filterGraph,neighborhood,layoutGraph,createCameraMotion};
})();
