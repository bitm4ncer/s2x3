const pre = document.getElementById("preloader");
const fill = document.getElementById("pre-fill");
const pct = document.getElementById("pre-pct");
const enter = document.getElementById("pre-enter");

let progress = 0;
const timer = setInterval(() => {
  progress = Math.min(100, progress + 6 + Math.random() * 11);
  if (fill) fill.style.width = `${progress}%`;
  if (pct) pct.textContent = `${Math.floor(progress)}%`;
  if (progress >= 100) {
    clearInterval(timer);
    setTimeout(() => pre?.classList.add("done"), 350);
  }
}, 110);

enter?.addEventListener("click", () => pre?.classList.add("done"));
pre?.addEventListener("transitionend", () => pre.remove());

const tabs = [...document.querySelectorAll(".tab")];
const ids = tabs
  .map((t) => t.getAttribute("href"))
  .filter((h) => h && h.startsWith("#") && h.length > 1);

const spy = new IntersectionObserver(
  (entries) => {
    for (const entry of entries) {
      if (!entry.isIntersecting) continue;
      const id = `#${entry.target.id}`;
      tabs.forEach((t) => t.classList.toggle("active", t.getAttribute("href") === id));
    }
  },
  { rootMargin: "-20% 0px -70% 0px" },
);

ids.forEach((id) => {
  const el = document.querySelector(id);
  if (el) spy.observe(el);
});
