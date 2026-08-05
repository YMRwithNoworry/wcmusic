const header = document.querySelector('[data-header]');
const reveals = document.querySelectorAll('.reveal');
const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;

window.addEventListener('scroll', () => {
  header.classList.toggle('scrolled', window.scrollY > 16);
}, { passive: true });

if (!reducedMotion && 'IntersectionObserver' in window) {
  const observer = new IntersectionObserver((entries) => {
    entries.forEach((entry) => {
      if (!entry.isIntersecting) return;
      entry.target.classList.add('visible');
      observer.unobserve(entry.target);
    });
  }, { threshold: 0.12 });
  reveals.forEach((element) => observer.observe(element));
} else {
  reveals.forEach((element) => element.classList.add('visible'));
}

const mobileDevice = /Android/i.test(navigator.userAgent);
if (mobileDevice) {
  const windowsCard = document.querySelector('[data-platform="windows"]');
  const androidCard = document.querySelector('[data-platform="android"]');
  const label = windowsCard.querySelector('.recommend-label');
  windowsCard.classList.remove('recommended');
  label.remove();
  androidCard.classList.add('recommended');
  androidCard.insertAdjacentHTML('afterbegin', '<div class="recommend-label">为此设备推荐</div>');
  const primary = document.querySelector('[data-primary-cta]');
  primary.href = 'https://github.com/YMRwithNoworry/wcmusic-releases/releases/download/v1.0.0/wcmusic-android-arm64.apk';
  primary.querySelector('span').textContent = '下载 Android 版';
}

document.querySelectorAll('details').forEach((detail) => {
  detail.addEventListener('toggle', () => {
    if (!detail.open) return;
    document.querySelectorAll('details[open]').forEach((other) => {
      if (other !== detail) other.open = false;
    });
  });
});

const customSelects = [...document.querySelectorAll('[data-custom-select]')];

function closeSelect(select, restoreFocus = false) {
  select.classList.remove('open');
  select.querySelector('[data-select-trigger]').setAttribute('aria-expanded', 'false');
  select.querySelectorAll('[role="option"]').forEach((option) => option.classList.remove('focused'));
  if (restoreFocus) select.querySelector('[data-select-trigger]').focus();
}

function openSelect(select) {
  customSelects.forEach((other) => {
    if (other !== select) closeSelect(other);
  });
  select.classList.add('open');
  select.querySelector('[data-select-trigger]').setAttribute('aria-expanded', 'true');
  const selected = select.querySelector('[aria-selected="true"]');
  (selected || select.querySelector('[role="option"]'))?.classList.add('focused');
}

function chooseOption(select, option) {
  const input = select.querySelector('[data-select-value]');
  const label = select.querySelector('[data-select-label]');
  select.querySelectorAll('[role="option"]').forEach((item) => item.setAttribute('aria-selected', String(item === option)));
  input.value = option.dataset.value;
  label.textContent = option.textContent.trim();
  label.classList.remove('placeholder');
  select.classList.remove('invalid');
  closeSelect(select, true);
}

customSelects.forEach((select) => {
  const trigger = select.querySelector('[data-select-trigger]');
  const options = [...select.querySelectorAll('[role="option"]')];
  trigger.addEventListener('click', () => select.classList.contains('open') ? closeSelect(select) : openSelect(select));
  options.forEach((option) => option.addEventListener('click', () => chooseOption(select, option)));

  select.addEventListener('keydown', (event) => {
    if (!['ArrowDown', 'ArrowUp', 'Home', 'End', 'Enter', ' ', 'Escape'].includes(event.key)) return;
    event.preventDefault();
    if (event.key === 'Escape') return closeSelect(select, true);
    const wasOpen = select.classList.contains('open');
    if (!wasOpen) openSelect(select);
    const current = options.findIndex((option) => option.classList.contains('focused'));
    if (event.key === 'Enter' || event.key === ' ') {
      if (wasOpen) chooseOption(select, options[Math.max(0, current)]);
      return;
    }
    options.forEach((option) => option.classList.remove('focused'));
    let next = current;
    if (event.key === 'Home') next = 0;
    if (event.key === 'End') next = options.length - 1;
    if (event.key === 'ArrowDown') next = (current + 1) % options.length;
    if (event.key === 'ArrowUp') next = (current - 1 + options.length) % options.length;
    options[next].classList.add('focused');
    options[next].scrollIntoView({ block: 'nearest' });
  });
});

document.addEventListener('click', (event) => {
  customSelects.forEach((select) => {
    if (!select.contains(event.target)) closeSelect(select);
  });
});

const feedbackForm = document.querySelector('[data-feedback-form]');
if (feedbackForm) {
  feedbackForm.addEventListener('submit', async (event) => {
    event.preventDefault();
    if (!feedbackForm.reportValidity()) return;

    const missingSelects = customSelects.filter((select) => !select.querySelector('[data-select-value]').value);
    customSelects.forEach((select) => select.classList.toggle('invalid', missingSelects.includes(select)));
    if (missingSelects.length) {
      openSelect(missingSelects[0]);
      missingSelects[0].querySelector('[data-select-trigger]').focus();
      return;
    }

    const submitButton = feedbackForm.querySelector('button[type="submit"]');
    const submitLabel = submitButton.querySelector('span');
    const status = feedbackForm.querySelector('[data-form-status]');
    submitButton.disabled = true;
    submitLabel.textContent = '正在发送';
    status.className = 'form-status';
    status.textContent = '';

    try {
      const response = await fetch(feedbackForm.action, {
        method: 'POST',
        body: new FormData(feedbackForm),
        headers: { Accept: 'application/json' },
      });
      if (!response.ok) throw new Error(`FormSubmit returned ${response.status}`);
      feedbackForm.reset();
      customSelects.forEach((select) => {
        select.querySelector('[data-select-value]').value = '';
        const label = select.querySelector('[data-select-label]');
        label.textContent = '请选择';
        label.classList.add('placeholder');
        select.querySelectorAll('[role="option"]').forEach((option) => option.setAttribute('aria-selected', 'false'));
      });
      status.className = 'form-status success';
      status.textContent = '已送出，感谢你帮助 WCMusic 变得更好。';
    } catch (error) {
      console.error(error);
      status.className = 'form-status error';
      status.textContent = '暂时没有发送成功，请稍后再试。';
    } finally {
      submitButton.disabled = false;
      submitLabel.textContent = '发送反馈';
    }
  });
}

window.addEventListener('DOMContentLoaded', () => {
  if (window.lucide) window.lucide.createIcons();
});

window.addEventListener('load', () => {
  if (!window.location.hash) return;
  document.querySelector(window.location.hash)?.scrollIntoView();
});
