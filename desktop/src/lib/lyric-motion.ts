// 两处歌词共用的换行缓动：起止速度和加速度均为零。
// 新换行从当前显示位置出发，取消旧动画后不会跳回上一行。
export function animateLyricPosition(
  from: number,
  to: number,
  update: (position: number) => void,
  immediate = false,
): () => void {
  if (immediate || Math.abs(to - from) < 0.5) {
    update(to);
    return () => {};
  }
  const duration = 1000;
  const start = performance.now();
  let frame = 0;
  const tick = (now: number) => {
    const progress = Math.min(1, Math.max(0, (now - start) / duration));
    const eased = progress ** 3 * (progress * (progress * 6 - 15) + 10);
    update(from + (to - from) * eased);
    if (progress < 1) frame = requestAnimationFrame(tick);
  };
  frame = requestAnimationFrame(tick);
  return () => cancelAnimationFrame(frame);
}
