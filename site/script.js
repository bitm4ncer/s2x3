(() => {
  const view = document.getElementById("portal-scroll");
  const bar = document.querySelector(".scrollbar");
  if (!view || !bar) return;

  const track = bar.querySelector(".sb-track");
  const thumb = bar.querySelector(".sb-thumb");
  const up = bar.querySelector(".sb-up");
  const down = bar.querySelector(".sb-down");
  const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");
  const behavior = () => (reducedMotion.matches ? "auto" : "smooth");

  let dragging = false;
  let dragStartY = 0;
  let dragStartTop = 0;

  const maxScroll = () => Math.max(0, view.scrollHeight - view.clientHeight);

  function sync() {
    const max = maxScroll();
    const trackHeight = track.clientHeight;
    if (!max || trackHeight <= 0) {
      bar.classList.add("is-idle");
      return;
    }

    bar.classList.remove("is-idle");
    const visibleRatio = view.clientHeight / view.scrollHeight;
    const thumbHeight = Math.max(38, Math.min(trackHeight, Math.round(trackHeight * visibleRatio)));
    const travel = Math.max(0, trackHeight - thumbHeight);
    const offset = max ? (view.scrollTop / max) * travel : 0;

    thumb.style.height = `${thumbHeight}px`;
    thumb.style.transform = `translateY(${offset}px)`;
  }

  function page(direction) {
    view.scrollBy({
      top: direction * Math.max(140, view.clientHeight * 0.86),
      behavior: behavior(),
    });
  }

  up.addEventListener("click", () => page(-1));
  down.addEventListener("click", () => page(1));

  track.addEventListener("pointerdown", (event) => {
    if (event.target === thumb) return;
    const thumbBox = thumb.getBoundingClientRect();
    page(event.clientY < thumbBox.top ? -1 : 1);
  });

  thumb.addEventListener("pointerdown", (event) => {
    event.preventDefault();
    dragging = true;
    dragStartY = event.clientY;
    dragStartTop = view.scrollTop;
    thumb.setPointerCapture(event.pointerId);
    document.body.classList.add("dragging-scrollbar");
  });

  thumb.addEventListener("pointermove", (event) => {
    if (!dragging) return;
    const max = maxScroll();
    const travel = track.clientHeight - thumb.offsetHeight;
    if (max <= 0 || travel <= 0) return;
    const delta = ((event.clientY - dragStartY) / travel) * max;
    view.scrollTop = dragStartTop + delta;
  });

  function endDrag(event) {
    if (!dragging) return;
    dragging = false;
    document.body.classList.remove("dragging-scrollbar");
    try {
      thumb.releasePointerCapture(event.pointerId);
    } catch (_) {
      /* pointer capture already released */
    }
  }

  thumb.addEventListener("pointerup", endDrag);
  thumb.addEventListener("pointercancel", endDrag);

  view.addEventListener("scroll", sync, { passive: true });

  if ("ResizeObserver" in window) {
    const observer = new ResizeObserver(sync);
    observer.observe(view);
    observer.observe(view.parentElement);
  }

  window.addEventListener("resize", sync);
  window.addEventListener("load", sync);
  if (document.fonts && document.fonts.ready) {
    document.fonts.ready.then(sync).catch(() => {});
  }

  function scrollToTarget(target, updateHash) {
    if (view.scrollHeight <= view.clientHeight + 2) {
      target.scrollIntoView({ behavior: behavior(), block: "start" });
    } else {
      const top =
        target.getBoundingClientRect().top -
        view.getBoundingClientRect().top +
        view.scrollTop -
        8;
      view.scrollTo({ top: Math.max(0, top), behavior: behavior() });
    }
    if (updateHash && history.replaceState) {
      history.replaceState(null, "", `#${target.id}`);
    }
  }

  document.querySelectorAll('a[href^="#"]').forEach((link) => {
    const hash = link.getAttribute("href");
    if (!hash || hash === "#") return;
    const target = document.querySelector(hash);
    if (!target || !view.contains(target)) return;

    link.addEventListener("click", (event) => {
      event.preventDefault();
      if (!link.closest(".layout")) {
        const board = document.querySelector(".board");
        if (board) board.scrollIntoView({ behavior: behavior(), block: "start" });
      }
      scrollToTarget(target, true);
    });
  });

  if (window.location.hash) {
    const initial = document.querySelector(window.location.hash);
    if (initial && view.contains(initial)) {
      window.setTimeout(() => scrollToTarget(initial, false), 0);
    }
  }

  sync();
})();
