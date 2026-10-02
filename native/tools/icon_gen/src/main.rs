//! 应用图标生成器：把 `assets/icons/app_icon.png` 转成多尺寸 Windows ICO。
//!
//! 为什么需要它：源图是一张 1254×1254 的插画，四周留有白边。之前的 ICO 是
//! 「整张图直接缩」的产物，于是任务栏/托盘里的 16~32px 图标既被白边挤小、又
//! 糊成一团。这里做了四件事：
//!
//! 1. 去掉圆角方框之外的白底（从图像四边 flood fill 判连通）改成透明，顺带裁掉白边；
//! 2. 按内容包围盒裁切，让图形在小尺寸下尽量占满画布；
//! 3. 用预乘 alpha 的 Lanczos3 缩放 + 轻微 unsharp，避免发虚；
//! 4. 16~32px 换成「角色特写 + 平色化」的小尺寸专用版本 —— 方寸之间塞下整张
//!    插画（背景漩涡、发丝、音符）必然是噪点，放大主体后耳机、眼睛才立得住。
//!
//! 小尺寸必须写成 BMP 条目（少部分 shell / 旧代码路径读不了 ICO 内的 PNG 条目），
//! 只有 96/128/256 这类「图标视图」尺寸用 PNG 省体积。
//!
//! 用法（仓库根目录）：
//!
//! ```bash
//! cargo +stable-x86_64-pc-windows-msvc run --release \
//!     --manifest-path native/tools/icon_gen/Cargo.toml -- --preview <目录>
//! ```
//!
//! 调试参数：`--grid <目录>` 导出 10% 标定网格；`--sheet <文件>` 导出各渲染方案对比图；
//! `--compare <旧ICO> <文件>` 导出新旧对比图。

use std::path::{Path, PathBuf};

use image::{Rgba, RgbaImage};

/// 源图（相对本 crate 的 CARGO_MANIFEST_DIR）。
const SOURCE_PNG: &str = "../../../assets/icons/app_icon.png";
/// 生成目标：GPUI 桌面端（build.rs 嵌入 exe）与 Flutter Windows 运行器各一份。
const OUTPUT_ICOS: &[&str] = &[
    "../../../assets/icons/app_icon.ico",
    "../../../windows/runner/resources/app_icon.ico",
];

/// BMP 条目尺寸：覆盖 100%~200% 缩放下的任务栏（16~32）、标题栏（16~24）、
/// Alt+Tab（32~64）与托盘（16~24），以及托盘在 175% 缩放下的 56。
const BMP_SIZES: &[u32] = &[16, 20, 24, 28, 32, 36, 40, 48, 56, 64];
/// PNG 条目尺寸：Windows「图标视图」用得到的大图。
const PNG_SIZES: &[u32] = &[96, 128, 256];
/// 到这个尺寸为止改用「角色特写 + 平色化」的小尺寸版本。
const SMALL_MAX: u32 = 32;
/// 小尺寸特写在裁边后画布里的范围（比例坐标，由 `--grid` 标定）。
const SMALL_REGION: CropRegion = CropRegion { x: 0.17, y: 0.14, w: 0.66, h: 0.66 };
/// 圆角轮廓半径（占边长比例），与插画自身的圆角方框一致。
const CORNER_RATIO: f32 = 0.14;

fn main() {
    let args = Args::parse();
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let source = manifest_dir.join(SOURCE_PNG);

    let src = image::open(&source)
        .unwrap_or_else(|e| panic!("无法读取源图 {}：{e}", source.display()))
        .to_rgba8();
    println!("源图 {}：{}×{}", source.display(), src.width(), src.height());

    let transparent = strip_background(&src);
    let cropped = crop_to_content(&transparent);
    println!(
        "去白底并裁边后：{}×{}（去掉 {}×{}）",
        cropped.width(),
        cropped.height(),
        src.width() - cropped.width(),
        src.height() - cropped.height()
    );

    if let Some(dir) = &args.grid {
        std::fs::create_dir_all(dir).expect("创建标定目录失败");
        dump_grid(&cropped, &dir.join("grid_full.png"));
        println!("标定网格已写入 {}", dir.display());
    }
    if let Some(path) = &args.sheet {
        contact_sheet(&cropped, path);
        println!("方案对比图已写入 {}", path.display());
    }
    if let Some((old_ico, path)) = &args.compare {
        compare_sheet(&cropped, old_ico, path);
        println!("新旧对比图已写入 {}", path.display());
    }

    let entries = build_entries(&cropped);
    let bytes = encode_ico(&entries);
    for relative in OUTPUT_ICOS {
        let output = manifest_dir.join(relative);
        std::fs::write(&output, &bytes)
            .unwrap_or_else(|e| panic!("写入 {} 失败：{e}", output.display()));
        println!("已写入 {}：{} 字节，{} 个尺寸", output.display(), bytes.len(), entries.len());
    }
    for entry in &entries {
        println!(
            "  {}×{} {}{}",
            entry.width,
            entry.height,
            if entry.png { "png" } else { "bmp" },
            if entry.width <= SMALL_MAX { "（特写版）" } else { "" }
        );
    }

    if let Some(dir) = &args.preview {
        std::fs::create_dir_all(dir).expect("创建预览目录失败");
        for &size in BMP_SIZES.iter().chain(PNG_SIZES) {
            let icon = render_icon(&cropped, size);
            let zoom = if size <= 64 { zoom_up(&icon, 8) } else { icon };
            zoom.save(dir.join(format!("icon_{size}.png"))).expect("写预览图失败");
        }
        println!("预览图已写入 {}", dir.display());
    }
}

/// 组装 ICO 的全部尺寸。
fn build_entries(cropped: &RgbaImage) -> Vec<IcoEntry> {
    let mut entries = Vec::new();
    for &size in BMP_SIZES {
        entries.push(IcoEntry::bmp(render_icon(cropped, size)));
    }
    for &size in PNG_SIZES {
        entries.push(IcoEntry::png(render_icon(cropped, size)));
    }
    entries
}

/// 某个尺寸最终用哪种渲染：小尺寸走特写版，大尺寸直接用整张插画。
fn render_icon(cropped: &RgbaImage, size: u32) -> RgbaImage {
    if size <= SMALL_MAX {
        flatten_small(&small_variant(cropped, size, SMALL_REGION))
    } else {
        render(cropped, size, size <= 64)
    }
}

/// 命令行参数。
#[derive(Default)]
struct Args {
    /// `--preview <目录>`：导出各尺寸预览 PNG（小尺寸放大 8 倍，方便肉眼检查）。
    preview: Option<PathBuf>,
    /// `--grid <目录>`：导出带 10% 网格的标定图，用来定位特写裁剪框。
    grid: Option<PathBuf>,
    /// `--sheet <文件>`：导出「小尺寸渲染方案」对比图。
    sheet: Option<PathBuf>,
    /// `--compare <旧ICO> <文件>`：导出新旧图标对比图。
    compare: Option<(PathBuf, PathBuf)>,
}

impl Args {
    fn parse() -> Self {
        let mut args = Self::default();
        let mut argv = std::env::args().skip(1);
        while let Some(arg) = argv.next() {
            match arg.as_str() {
                "--preview" => args.preview = argv.next().map(PathBuf::from),
                "--grid" => args.grid = argv.next().map(PathBuf::from),
                "--sheet" => args.sheet = argv.next().map(PathBuf::from),
                "--compare" => {
                    let old = argv.next().map(PathBuf::from);
                    let out = argv.next().map(PathBuf::from);
                    if let (Some(old), Some(out)) = (old, out) {
                        args.compare = Some((old, out));
                    }
                }
                other => eprintln!("忽略未知参数：{other}"),
            }
        }
        args
    }
}

/// 特写裁剪范围（比例坐标）。
#[derive(Clone, Copy)]
struct CropRegion {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

/// 把四边连通的白底改成透明。
///
/// 只处理与图像边界连通、且接近纯白的像素，插画内部的白色（衣领、音符）不会
/// 受影响；圆角边缘的抗锯齿像素按「白底占比」折算 alpha，避免留下白边。
fn strip_background(src: &RgbaImage) -> RgbaImage {
    let (w, h) = src.dimensions();
    let mut out = src.clone();
    let mut visited = vec![false; (w * h) as usize];
    let mut stack: Vec<(u32, u32)> = Vec::new();

    let push = |x: u32, y: u32, stack: &mut Vec<(u32, u32)>, visited: &mut Vec<bool>| {
        let index = (y * w + x) as usize;
        if !visited[index] && is_background(src.get_pixel(x, y)) {
            visited[index] = true;
            stack.push((x, y));
        }
    };

    for x in 0..w {
        push(x, 0, &mut stack, &mut visited);
        push(x, h - 1, &mut stack, &mut visited);
    }
    for y in 0..h {
        push(0, y, &mut stack, &mut visited);
        push(w - 1, y, &mut stack, &mut visited);
    }

    while let Some((x, y)) = stack.pop() {
        let pixel = *src.get_pixel(x, y);
        let whiteness = background_ratio(pixel);
        let alpha = ((1.0 - whiteness) * 255.0).round().clamp(0.0, 255.0) as u8;
        out.put_pixel(x, y, Rgba([pixel[0], pixel[1], pixel[2], alpha]));

        for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
            let nx = x as i32 + dx;
            let ny = y as i32 + dy;
            if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                continue;
            }
            push(nx as u32, ny as u32, &mut stack, &mut visited);
        }
    }
    out
}

/// 像素是否算「白底」。
fn is_background(pixel: &Rgba<u8>) -> bool {
    background_ratio(*pixel) > 0.55
}

/// 白底占比：0 = 完全不是白底，1 = 纯白。
fn background_ratio(pixel: Rgba<u8>) -> f32 {
    let min = pixel[0].min(pixel[1]).min(pixel[2]) as f32;
    let max = pixel[0].max(pixel[1]).max(pixel[2]) as f32;
    if max - min > 12.0 {
        // 有明显色彩，不是白底。
        return 0.0;
    }
    ((min - 214.0) / (255.0 - 214.0)).clamp(0.0, 1.0)
}

/// 裁到不透明内容的包围盒（留 1% 余量，保持正方）。
fn crop_to_content(src: &RgbaImage) -> RgbaImage {
    let (w, h) = src.dimensions();
    let (mut left, mut top, mut right, mut bottom) = (w, h, 0u32, 0u32);
    for y in 0..h {
        for x in 0..w {
            if src.get_pixel(x, y)[3] > 8 {
                left = left.min(x);
                top = top.min(y);
                right = right.max(x);
                bottom = bottom.max(y);
            }
        }
    }
    if left >= right || top >= bottom {
        return src.clone();
    }
    let side = ((right - left + 1).max(bottom - top + 1) as f32 * 1.02).round() as u32;
    let side = side.min(w).min(h);
    let center_x = (left + right) / 2;
    let center_y = (top + bottom) / 2;
    let left = center_x.saturating_sub(side / 2).min(w - side);
    let top = center_y.saturating_sub(side / 2).min(h - side);
    image::imageops::crop_imm(src, left, top, side, side).to_image()
}

/// 整张插画按目标尺寸渲染：预乘 alpha 缩放 → 小尺寸补一次 unsharp。
fn render(src: &RgbaImage, size: u32, sharpen: bool) -> RgbaImage {
    let scaled = premultiplied_resize(src, size, size);
    if sharpen { unsharp(&scaled, 0.6) } else { scaled }
}

/// 小尺寸特写：裁到角色头部、按尺寸缩放，再套上与整图一致的圆角轮廓。
///
/// 16~32px 塞下整张插画必然是噪点（背景漩涡、发丝、音符都会糊在一起），
/// 放大主体后耳机、眼睛这些大色块才立得住。
fn small_variant(src: &RgbaImage, size: u32, region: CropRegion) -> RgbaImage {
    let (canvas_w, canvas_h) = src.dimensions();
    // 方裁：保持宽高一致，避免特写被拉变形。
    let side = (region.w.min(region.h) * canvas_w as f32).round().max(1.0) as u32;
    let max_x = canvas_w.saturating_sub(side);
    let max_y = canvas_h.saturating_sub(side);
    let x = ((region.x * canvas_w as f32).round() as u32).min(max_x);
    let y = ((region.y * canvas_h as f32).round() as u32).min(max_y);
    let view = image::imageops::crop_imm(src, x, y, side, side).to_image();

    let scaled = premultiplied_resize(&view, size, size);
    let sharpened = if size <= 32 { unsharp(&scaled, 0.45) } else { scaled };
    apply_rounded_mask(&sharpened, CORNER_RATIO)
}

/// 套一个圆角矩形遮罩（4×4 子采样抗锯齿），让特写版的轮廓与原图圆角方框一致。
fn apply_rounded_mask(src: &RgbaImage, radius_ratio: f32) -> RgbaImage {
    const SAMPLES: u32 = 4;
    let (w, h) = src.dimensions();
    let radius = (radius_ratio * w as f32).max(1.0);
    let mut out = src.clone();
    for y in 0..h {
        for x in 0..w {
            let mut inside = 0u32;
            for sy in 0..SAMPLES {
                for sx in 0..SAMPLES {
                    let px = x as f32 + (sx as f32 + 0.5) / SAMPLES as f32;
                    let py = y as f32 + (sy as f32 + 0.5) / SAMPLES as f32;
                    if rounded_rect_contains(px, py, w as f32, h as f32, radius) {
                        inside += 1;
                    }
                }
            }
            if inside == 0 {
                out.put_pixel(x, y, Rgba([0, 0, 0, 0]));
            } else if inside < SAMPLES * SAMPLES {
                let pixel = *out.get_pixel(x, y);
                let coverage = inside as f32 / (SAMPLES * SAMPLES) as f32;
                out.put_pixel(
                    x,
                    y,
                    Rgba([pixel[0], pixel[1], pixel[2], (pixel[3] as f32 * coverage).round() as u8]),
                );
            }
        }
    }
    out
}

/// 点是否落在圆角矩形内。
fn rounded_rect_contains(px: f32, py: f32, w: f32, h: f32, radius: f32) -> bool {
    if px < 0.0 || py < 0.0 || px > w || py > h {
        return false;
    }
    let cx = px.min(w - px);
    let cy = py.min(h - py);
    if cx >= radius || cy >= radius {
        return true;
    }
    let dx = radius - cx;
    let dy = radius - cy;
    dx * dx + dy * dy <= radius * radius
}

/// 小尺寸「平色化」：小图标里渐变的灰发会糊成灰雾，提饱和、提对比并按 12 级量化后
/// 剩下的平色块反而更清楚。
fn flatten_small(src: &RgbaImage) -> RgbaImage {
    let mut out = src.clone();
    for pixel in out.pixels_mut() {
        if pixel[3] == 0 {
            continue;
        }
        let luminance = 0.299 * pixel[0] as f32 + 0.587 * pixel[1] as f32 + 0.114 * pixel[2] as f32;
        let channels = [pixel[0] as f32, pixel[1] as f32, pixel[2] as f32].map(|channel| {
            let value = luminance + (channel - luminance) * 1.4;
            let value = (value - 128.0) * 1.18 + 128.0 + 4.0;
            let steps = 12.0;
            (value / 255.0 * (steps - 1.0)).round() / (steps - 1.0) * 255.0
        });
        pixel[0] = channels[0].clamp(0.0, 255.0).round() as u8;
        pixel[1] = channels[1].clamp(0.0, 255.0).round() as u8;
        pixel[2] = channels[2].clamp(0.0, 255.0).round() as u8;
    }
    out
}

/// 预乘 alpha 的 Lanczos3 缩放。
///
/// 直接对 RGBA 四个通道各自缩放，会把透明像素的颜色掺进来，边缘出现彩边/暗边；
/// 转成预乘 alpha 再缩可以避免这一点。
fn premultiplied_resize(src: &RgbaImage, width: u32, height: u32) -> RgbaImage {
    let mut premultiplied = RgbaImage::new(src.width(), src.height());
    for (src_pixel, dst_pixel) in src.pixels().zip(premultiplied.pixels_mut()) {
        let alpha = src_pixel[3] as f32 / 255.0;
        *dst_pixel = Rgba([
            (src_pixel[0] as f32 * alpha).round() as u8,
            (src_pixel[1] as f32 * alpha).round() as u8,
            (src_pixel[2] as f32 * alpha).round() as u8,
            src_pixel[3],
        ]);
    }
    let scaled =
        image::imageops::resize(&premultiplied, width, height, image::imageops::FilterType::Lanczos3);

    let mut out = RgbaImage::new(width, height);
    for (src_pixel, dst_pixel) in scaled.pixels().zip(out.pixels_mut()) {
        let alpha = src_pixel[3] as f32 / 255.0;
        if alpha <= 0.0001 {
            *dst_pixel = Rgba([0, 0, 0, 0]);
            continue;
        }
        let unmultiply = |channel: u8| ((channel as f32 / alpha).round()).clamp(0.0, 255.0) as u8;
        *dst_pixel = Rgba([
            unmultiply(src_pixel[0]),
            unmultiply(src_pixel[1]),
            unmultiply(src_pixel[2]),
            src_pixel[3],
        ]);
    }
    out
}

/// 轻微 unsharp mask：小尺寸缩完会发虚，往回提一点边缘。
fn unsharp(src: &RgbaImage, amount: f32) -> RgbaImage {
    let blurred = image::imageops::blur(src, 0.6);
    let mut out = src.clone();
    for ((src_pixel, blurred_pixel), dst_pixel) in
        src.pixels().zip(blurred.pixels()).zip(out.pixels_mut())
    {
        if src_pixel[3] == 0 {
            continue;
        }
        let mut channels = [0u8; 4];
        for channel in 0..3 {
            let value = src_pixel[channel] as f32
                + (src_pixel[channel] as f32 - blurred_pixel[channel] as f32) * amount;
            channels[channel] = value.round().clamp(0.0, 255.0) as u8;
        }
        channels[3] = src_pixel[3];
        *dst_pixel = Rgba(channels);
    }
    out
}

/// 最近邻放大，只为肉眼检查小尺寸（不参与 ICO 生成）。
fn zoom_up(src: &RgbaImage, factor: u32) -> RgbaImage {
    let mut out = RgbaImage::new(src.width() * factor, src.height() * factor);
    for y in 0..out.height() {
        for x in 0..out.width() {
            out.put_pixel(x, y, *src.get_pixel(x / factor, y / factor));
        }
    }
    out
}

/// 各渲染方案的小尺寸对比图（整图 / 整图平色化 / 特写 / 特写平色化）。
fn contact_sheet(src: &RgbaImage, path: &Path) {
    const SIZES: &[u32] = &[16, 20, 24, 32];
    const ZOOM: u32 = 4;
    const CELL: u32 = 32 * ZOOM + 12;
    let mut sheet = RgbaImage::from_pixel(
        CELL * SIZES.len() as u32,
        CELL * 4,
        Rgba([28, 28, 32, 255]),
    );
    for (variant, mode) in ["full", "full-flat", "head", "head-flat"].iter().enumerate() {
        for (column, &size) in SIZES.iter().enumerate() {
            let base = if mode.starts_with("head") {
                small_variant(src, size, SMALL_REGION)
            } else {
                render(src, size, true)
            };
            let icon = if mode.ends_with("flat") { flatten_small(&base) } else { base };
            let origin_x = column as u32 * CELL + (CELL - size * ZOOM) / 2;
            let origin_y = variant as u32 * CELL + (CELL - size * ZOOM) / 2;
            blit(&mut sheet, &zoom_up(&icon, ZOOM), origin_x, origin_y);
        }
    }
    sheet.save(path).expect("写对比图失败");
}

/// 新旧图标对比图：上排来自旧 ICO，下排是当前渲染。
fn compare_sheet(src: &RgbaImage, old_ico: &Path, path: &Path) {
    const SIZES: &[u32] = &[16, 24, 32];
    let old = read_ico(old_ico).unwrap_or_else(|e| panic!("读取旧 ICO 失败：{e}"));
    let mut old_icons = Vec::new();
    for &size in SIZES {
        let entry = old
            .iter()
            .find(|entry| entry.width == size)
            .unwrap_or_else(|| panic!("旧 ICO 缺少 {size}px 条目"));
        let decoded = if entry.png {
            image::load_from_memory(&entry.data).expect("解码旧 ICO 的 PNG 条目失败").to_rgba8()
        } else {
            panic!("旧 ICO 的 {size}px 条目不是 PNG，未实现 DIB 解码");
        };
        old_icons.push((size, decoded));
    }
    let new_icons: Vec<(u32, RgbaImage)> =
        SIZES.iter().map(|&size| (size, render_icon(src, size))).collect();

    const ZOOM: u32 = 4;
    const GAP: u32 = 12;
    let rows = [&old_icons, &new_icons];
    let cell_width = 32 * ZOOM + GAP;
    let cell_height = 32 * ZOOM + GAP;
    let mut sheet = RgbaImage::from_pixel(
        cell_width * SIZES.len() as u32,
        cell_height * rows.len() as u32,
        Rgba([28, 28, 32, 255]),
    );
    for (row, icons) in rows.iter().enumerate() {
        for (column, (size, icon)) in icons.iter().enumerate() {
            let origin_x = column as u32 * cell_width + (cell_width - size * ZOOM) / 2;
            let origin_y = row as u32 * cell_height + (cell_height - size * ZOOM) / 2;
            blit(&mut sheet, &zoom_up(icon, ZOOM), origin_x, origin_y);
        }
    }
    sheet.save(path).expect("写对比图失败");
}

/// 把带 alpha 的小图合成到大图上。
fn blit(canvas: &mut RgbaImage, sprite: &RgbaImage, x: u32, y: u32) {
    for (sx, sy, pixel) in sprite.enumerate_pixels() {
        if pixel[3] == 0 {
            continue;
        }
        let (cx, cy) = (x + sx, y + sy);
        if cx >= canvas.width() || cy >= canvas.height() {
            continue;
        }
        let under = *canvas.get_pixel(cx, cy);
        let alpha = pixel[3] as f32 / 255.0;
        let blend = |fg: u8, bg: u8| (fg as f32 * alpha + bg as f32 * (1.0 - alpha)).round() as u8;
        canvas.put_pixel(
            cx,
            cy,
            Rgba([
                blend(pixel[0], under[0]),
                blend(pixel[1], under[1]),
                blend(pixel[2], under[2]),
                255,
            ]),
        );
    }
}

/// 导出 10% 间隔网格的标定图（内容合成到中灰底上，透明区域也看得见）。
fn dump_grid(src: &RgbaImage, path: &Path) {
    const CANVAS: u32 = 640;
    let scaled = premultiplied_resize(src, CANVAS, CANVAS);
    let mut canvas = RgbaImage::from_pixel(CANVAS, CANVAS, Rgba([96, 96, 104, 255]));
    for (x, y, pixel) in scaled.enumerate_pixels() {
        if pixel[3] == 0 {
            continue;
        }
        let alpha = pixel[3] as f32 / 255.0;
        let under = canvas.get_pixel(x, y);
        let blend = |fg: u8, bg: u8| (fg as f32 * alpha + bg as f32 * (1.0 - alpha)).round() as u8;
        canvas.put_pixel(
            x,
            y,
            Rgba([
                blend(pixel[0], under[0]),
                blend(pixel[1], under[1]),
                blend(pixel[2], under[2]),
                255,
            ]),
        );
    }
    let step = CANVAS / 10;
    for i in 1..10 {
        let color = if i % 5 == 0 { Rgba([255, 64, 64, 255]) } else { Rgba([255, 255, 0, 255]) };
        for t in 0..CANVAS {
            canvas.put_pixel(i * step, t, color);
            canvas.put_pixel(t, i * step, color);
        }
    }
    canvas.save(path).expect("写标定图失败");
}

/// ICO 里的一个尺寸。
struct IcoEntry {
    width: u32,
    height: u32,
    png: bool,
    data: Vec<u8>,
}

impl IcoEntry {
    /// 32bpp BGRA 的 BMP 条目（Windows 传统格式，兼容性最好）。
    fn bmp(image: RgbaImage) -> Self {
        let (width, height) = image.dimensions();
        let mut data = Vec::with_capacity((width * height * 4 + width * height / 8) as usize + 40);
        // BITMAPINFOHEADER：高度写两倍（XOR 位图 + AND 掩码）。
        data.extend_from_slice(&40u32.to_le_bytes());
        data.extend_from_slice(&(width as i32).to_le_bytes());
        data.extend_from_slice(&((height * 2) as i32).to_le_bytes());
        data.extend_from_slice(&1u16.to_le_bytes());
        data.extend_from_slice(&32u16.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes());
        data.extend_from_slice(&(width * height * 4).to_le_bytes());
        data.extend_from_slice(&[0u8; 16]);
        // 像素数据自下而上、BGRA 顺序。
        for y in (0..height).rev() {
            for x in 0..width {
                let pixel = image.get_pixel(x, y);
                data.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
            }
        }
        // AND 掩码：32bpp 下透明度由 alpha 决定，这里全 0，每行按 4 字节对齐。
        let mask_stride = width.div_ceil(32) * 4;
        data.extend(std::iter::repeat_n(0u8, (mask_stride * height) as usize));
        Self { width, height, png: false, data }
    }

    /// PNG 条目：只为 96/128/256 这类大图省体积。
    fn png(image: RgbaImage) -> Self {
        let (width, height) = image.dimensions();
        let mut data = Vec::new();
        image
            .write_to(&mut std::io::Cursor::new(&mut data), image::ImageFormat::Png)
            .expect("PNG 编码失败");
        Self { width, height, png: true, data }
    }
}

/// 组装 ICO 文件。
fn encode_ico(entries: &[IcoEntry]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * entries.len() as u32;
    for entry in entries {
        out.push(if entry.width >= 256 { 0 } else { entry.width as u8 });
        out.push(if entry.height >= 256 { 0 } else { entry.height as u8 });
        out.push(0);
        out.push(0);
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&32u16.to_le_bytes());
        out.extend_from_slice(&(entry.data.len() as u32).to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        offset += entry.data.len() as u32;
    }
    for entry in entries {
        out.extend_from_slice(&entry.data);
    }
    out
}

/// 读取 ICO 的目录（只用来做新旧对比，不校验内容）。
fn read_ico(path: &Path) -> Result<Vec<IcoEntry>, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    if bytes.len() < 6 {
        return Err("文件太短".to_owned());
    }
    let count = u16::from_le_bytes([bytes[4], bytes[5]]) as usize;
    let mut entries = Vec::new();
    for index in 0..count {
        let base = 6 + 16 * index;
        if base + 16 > bytes.len() {
            return Err("目录项越界".to_owned());
        }
        let width = if bytes[base] == 0 { 256 } else { bytes[base] as u32 };
        let height = if bytes[base + 1] == 0 { 256 } else { bytes[base + 1] as u32 };
        let size = u32::from_le_bytes([
            bytes[base + 8],
            bytes[base + 9],
            bytes[base + 10],
            bytes[base + 11],
        ]) as usize;
        let offset = u32::from_le_bytes([
            bytes[base + 12],
            bytes[base + 13],
            bytes[base + 14],
            bytes[base + 15],
        ]) as usize;
        if offset + size > bytes.len() {
            return Err("条目数据越界".to_owned());
        }
        let data = bytes[offset..offset + size].to_vec();
        let png = data.starts_with(&[0x89, b'P', b'N', b'G']);
        entries.push(IcoEntry { width, height, png, data });
    }
    Ok(entries)
}