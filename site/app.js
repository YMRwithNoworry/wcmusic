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

const feedbackForm = document.querySelector('[data-feedback-form]');
if (feedbackForm) {
  feedbackForm.addEventListener('submit', async (event) => {
    event.preventDefault();
    if (!feedbackForm.reportValidity()) return;

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
