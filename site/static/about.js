/* A quiet, input-driven orbit. At rest there is no animation loop. */
(() => {
  let dispose;

  function initOrbit() {
    dispose?.();
    const art = document.querySelector('.profile-art');
    if (!art) return;

    const primary = art.querySelector('.art-point-primary');
    const secondary = art.querySelector('.art-point-secondary');
    const halo = art.querySelector('.art-point-halo');
    const inner = art.querySelector('.art-inner');
    const reducedMotion = matchMedia('(prefers-reduced-motion: reduce)');
    const finePointer = matchMedia('(hover: hover) and (pointer: fine)');
    const events = new AbortController();
    let frame = 0;
    let lastTime = 0;
    let inView = true;
    let hovering = false;
    let targetX = 0;
    let targetY = 0;
    let x = 0;
    let y = 0;
    let energy = 0;
    let speed = 0;
    let impulse = 0;
    let phase = 0;

    function draw() {
      art.style.setProperty('--art-tilt-x', `${-y * 12}deg`);
      art.style.setProperty('--art-tilt-y', `${x * 14}deg`);
      art.style.setProperty('--art-shift-x', `${x * 5}px`);
      art.style.setProperty('--art-shift-y', `${y * 5}px`);
      art.style.setProperty('--art-mark-turn', `${x * 9}deg`);
      art.style.setProperty('--art-energy', energy.toFixed(3));
      inner.setAttribute('transform', `rotate(${38 + x * 12 + y * 6} 160 160)`);
      const angle = phase - .71;
      const px = 160 + Math.cos(angle) * 112;
      const py = 160 + Math.sin(angle) * 112;
      for (const point of [primary, halo]) {
        point.setAttribute('cx', px.toFixed(3));
        point.setAttribute('cy', py.toFixed(3));
      }
      secondary.setAttribute('cx', (160 + Math.cos(1.75 - phase * .8) * 62).toFixed(3));
      secondary.setAttribute('cy', (160 + Math.sin(1.75 - phase * .8) * 112).toFixed(3));
    }

    function tick(now) {
      frame = 0;
      const dt = Math.min((now - (lastTime || now)) / 1000, .05);
      lastTime = now;
      const ease = 1 - Math.exp(-dt * 7);
      x += (targetX - x) * ease;
      y += (targetY - y) * ease;
      const targetEnergy = hovering ? .65 : Math.min(impulse / 2, 1);
      energy += (targetEnergy - energy) * ease;
      speed += ((hovering ? .3 : 0) - speed) * ease;
      impulse *= Math.exp(-dt * 2.2);
      // The two paths repeat together after five outer revolutions.
      phase = (phase + (speed + impulse) * dt) % (Math.PI * 10);
      draw();
      if (hovering || Math.abs(x - targetX) + Math.abs(y - targetY) > .001 ||
          energy > .001 || speed > .001 || impulse > .001) {
        frame = requestAnimationFrame(tick);
      } else {
        x = targetX;
        y = targetY;
        energy = speed = impulse = 0;
        lastTime = 0;
        draw();
      }
    }

    function start() {
      if (!frame && inView && !document.hidden && !reducedMotion.matches) {
        lastTime = 0;
        frame = requestAnimationFrame(tick);
      }
    }

    function leave() {
      hovering = false;
      targetX = targetY = 0;
      start();
    }

    function stop() {
      cancelAnimationFrame(frame);
      frame = lastTime = 0;
      hovering = false;
      targetX = targetY = x = y = energy = speed = impulse = 0;
      draw();
    }

    function track(event) {
      if (event.pointerType === 'touch' || !finePointer.matches || reducedMotion.matches) return;
      const rect = art.getBoundingClientRect();
      targetX = Math.max(-1, Math.min(1, (event.clientX - rect.left) / rect.width * 2 - 1));
      targetY = Math.max(-1, Math.min(1, (event.clientY - rect.top) / rect.height * 2 - 1));
      hovering = true;
      start();
    }

    const on = (target, type, handler) => target.addEventListener(type, handler, { signal: events.signal });
    on(art, 'pointerenter', track);
    on(art, 'pointermove', track);
    on(art, 'pointerleave', leave);
    on(art, 'pointercancel', leave);
    on(art, 'click', () => {
      if (reducedMotion.matches) return;
      impulse = Math.min(impulse + 2.8, 4);
      start();
    });
    on(art, 'blur', leave);
    on(document, 'visibilitychange', () => { if (document.hidden) stop(); });
    on(window, 'pagehide', stop);
    on(reducedMotion, 'change', stop);
    on(finePointer, 'change', stop);

    const observer = new IntersectionObserver(entries => {
      inView = entries[0].isIntersecting;
      if (!inView) stop();
    });
    observer.observe(art);
    dispose = () => {
      stop();
      observer.disconnect();
      events.abort();
    };
  }

  initOrbit();
  document.addEventListener('blog:preview-updated', initOrbit);
})();
