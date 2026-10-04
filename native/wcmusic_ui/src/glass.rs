//! 歌曲详情页的液态玻璃（liquid glass）视觉。
//!
//! ## 参考实现与本项目的差距
//!
//! 参考实现 <https://github.com/ybouane/liquidglass> 是一套 WebGL 方案：先用
//! `html-to-image` 把面板背后的页面光栅化成纹理，做一遍分离式高斯模糊，再在片元
//! 着色器里做圆角矩形 SDF、双面折射（IOR 1.5）、色散、边缘加权的模糊混合、Fresnel、
//! 多光源高光、内描边与投影。它依赖 DOM、canvas 2D 与 WebGL。
//!
//! 本项目是 Rust + GPUI（wgpu / DirectX 12）原生渲染：没有 DOM，也没有「读回帧缓冲」
//! 或「自定义着色器」的接口，而这两条正是折射与背景模糊的前提；引入 WebView 又等于
//! 塞进一整套新渲染栈。所以这里**按它的原理在本项目内实现**，着色器里的光照与合成
//! 阶段逐条对应，只有「采样背景」换成近似手段：
//!
//! | 参考实现 | 这里 |
//! | --- | --- |
//! | 圆角矩形 SDF + 抗锯齿遮罩 | `rounded()` + `overflow_hidden()` |
//! | 玻璃体（冷色调、亮度/饱和度） | 半透明渐变底色 + 极淡的冷色调覆盖层 |
//! | 内描边（`topBias`：顶部更亮） | 一上一下两条 inset 阴影 |
//! | Fresnel 边缘提亮 + 内发光 | 大范围、低透明度的 inset 亮色阴影 |
//! | 多光源 Blinn-Phong 高光 | 斜向白色渐变高光条 |
//! | 外投影 + 接触阴影 | 两条外阴影（大范围柔光 + 贴地阴影） |
//! | 背景高斯模糊 | 48px 缩略图放大（`blurred_artwork_path`），双线性插值即模糊 |
//! | 背景折射（按 UV 位移采样） | 面板内叠一份**放大**的同一张封面 |
//!
//! 最后一条是与参考实现差距最大的地方：拿不到背景像素就没有真正的 UV 位移折射。
//! 近似做法能让玻璃里的内容相对外部放大（透镜最直观的感知特征），但不会出现参考
//! 实现那种沿边缘弯曲的色散。

use gpui_kit as gpui;

use gpui::{
    BoxShadow, Div, Hsla, ObjectFit, div, hsla, img, linear_color_stop, linear_gradient, prelude::*,
    px,
};

/// 玻璃面板的配色与圆角。
#[derive(Clone, Copy)]
pub struct GlassPanel {
    pub radius: f32,
    /// 玻璃体底色：上浅下深，玻璃有厚度、光从上方来。
    pub tint_top: Hsla,
    pub tint_bottom: Hsla,
    /// 高光、内描边与内发光用的亮色。
    pub light: Hsla,
    /// 投影与底部内描边用的暗色。
    pub shade: Hsla,
}

impl GlassPanel {
    /// 由主题色推出一套玻璃配色。
    ///
    /// `surface` 是主题的面板色，`foreground` 用来当高光色（浅色主题下是深色，
    /// 高光仍然成立：玻璃边缘反射的是环境光，跟主题明暗无关）。
    pub fn new(surface: Hsla, foreground: Hsla, radius: f32) -> Self {
        Self {
            radius,
            tint_top: surface.opacity(0.60),
            tint_bottom: surface.opacity(0.34),
            light: foreground,
            shade: hsla(0.0, 0.0, 0.0, 1.0),
        }
    }
}

/// 一块玻璃面板。
///
/// 返回的 `Div` 可以继续 `.child(...)` 放内容——玻璃的各个光学图层是绝对定位的，
/// 既不参与布局，也不会裁剪调用方放进去的内容（歌词要能滚动）。
///
/// `refraction` 是「玻璃里看到的背景」：传封面图就会叠一份放大、低透明度的副本，
/// 对应参考实现里按折射后的 UV 采样背景那一步。
pub fn glass_panel(panel: GlassPanel, refraction: Option<std::path::PathBuf>) -> Div {
    let radius = px(panel.radius);
    let mut body = div()
        .absolute()
        .inset_0()
        .rounded(radius)
        .overflow_hidden()
        // 玻璃体：上浅下深的半透明底。
        .bg(linear_gradient(
            155.0,
            linear_color_stop(panel.tint_top, 0.0),
            linear_color_stop(panel.tint_bottom, 1.0),
        ));
    if let Some(path) = refraction {
        // 折射：同一张封面在面板里被放大（负 inset 让图片比面板大一圈），
        // 于是玻璃里的内容相对外部是放大的——这就是透镜最直观的感知特征。
        body = body.child(
            img(path)
                .absolute()
                .inset(px(-panel.radius * 0.5))
                .object_fit(ObjectFit::Cover)
                .opacity(0.22),
        );
    }
    body = body
        // 冷色调：参考实现最后乘的 vec3(0.92, 0.95, 1.05)，这里用一层极淡的蓝。
        .child(div().absolute().inset_0().bg(hsla(0.58, 0.50, 0.70, 0.05)))
        // 高光条：多光源 specular 的近似，斜向铺在左上角。
        .child(div().absolute().inset_0().bg(linear_gradient(
            135.0,
            linear_color_stop(panel.light.opacity(0.14), 0.0),
            linear_color_stop(panel.light.opacity(0.0), 0.42),
        )))
        // 折射边缘：右下角压暗、左上角略亮，模拟透镜边缘的光线压缩。
        .child(div().absolute().inset_0().bg(linear_gradient(
            135.0,
            linear_color_stop(panel.light.opacity(0.06), 0.0),
            linear_color_stop(panel.shade.opacity(0.20), 1.0),
        )));

    div()
        .relative()
        .rounded(radius)
        .shadow(vec![
            // 外投影：大范围柔光（outerShadow）。
            BoxShadow::new(px(0.0), px(20.0), panel.shade.opacity(0.42))
                .blur_radius(px(44.0))
                .spread_radius(px(-8.0)),
            // 贴地的接触阴影（contactShadow）。
            BoxShadow::new(px(0.0), px(3.0), panel.shade.opacity(0.32)).blur_radius(px(9.0)),
            // 内描边：顶部亮、底部暗（着色器里带 topBias 的 innerStroke）。
            BoxShadow::new(px(0.0), px(1.5), panel.light.opacity(0.50))
                .blur_radius(px(2.0))
                .inset(),
            BoxShadow::new(px(0.0), px(-1.5), panel.shade.opacity(0.40))
                .blur_radius(px(3.0))
                .inset(),
            // 内发光：光线在玻璃体里散射（Fresnel 边缘提亮的近似）。
            BoxShadow::new(px(0.0), px(0.0), panel.light.opacity(0.09))
                .blur_radius(px(30.0))
                .spread_radius(px(-12.0))
                .inset(),
        ])
        .child(body)
}

/// 详情页背景：底色 + 放大压暗的封面 + 渐变压暗。
///
/// 没有封面时只画底色与渐变——这就是降级路径，不会出现空白区域；
/// 渐变压暗无论有没有封面都铺，保证上层的玻璃面板与文字始终可读。
pub fn artwork_backdrop(
    artwork: Option<std::path::PathBuf>,
    base: Hsla,
    shade: Hsla,
) -> Div {
    let mut backdrop = div().absolute().inset_0().bg(base);
    if let Some(path) = artwork {
        backdrop = backdrop.child(
            img(path)
                .absolute()
                .inset_0()
                .size_full()
                .object_fit(ObjectFit::Cover)
                .opacity(0.55),
        );
    }
    backdrop.child(
        div().absolute().inset_0().bg(linear_gradient(
            180.0,
            linear_color_stop(shade.opacity(0.40), 0.0),
            linear_color_stop(shade.opacity(0.78), 1.0),
        )),
    )
}
#[cfg(test)]
mod tests {
    use super::*;

    fn surface() -> Hsla {
        hsla(0.0, 0.0, 0.18, 1.0)
    }

    fn foreground() -> Hsla {
        hsla(0.0, 0.0, 0.95, 1.0)
    }

    /// 玻璃体必须真的半透明：不透明就看不到背后的封面，也就没有玻璃感。
    #[test]
    fn the_glass_body_stays_translucent() {
        let panel = GlassPanel::new(surface(), foreground(), 20.0);
        assert!(panel.tint_top.a < 1.0, "玻璃体必须是半透明的");
        assert!(
            panel.tint_bottom.a < panel.tint_top.a,
            "上浅下深才有厚度感"
        );
        assert_eq!(panel.radius, 20.0);
    }

    /// 降级路径：没有封面（或封面模糊图还没生成）时背景仍然要有底色，
    /// 不能留下空白区域。
    #[test]
    fn backdrop_without_artwork_still_paints_a_base_colour() {
        let mut backdrop = artwork_backdrop(None, surface(), hsla(0.0, 0.0, 0.0, 1.0));
        assert!(
            backdrop.style().background.is_some(),
            "没有封面时也必须铺底色"
        );
    }

    /// 面板在「有折射内容」和「没有折射内容」两种情况下都要能建出来。
    #[test]
    fn glass_panel_builds_with_and_without_refraction() {
        let style = GlassPanel::new(surface(), foreground(), 18.0);
        let mut plain = glass_panel(style, None);
        assert!(plain.style().box_shadow.is_some());
        let mut with_image = glass_panel(style, Some(std::path::PathBuf::from("cover.jpg")));
        assert!(with_image.style().box_shadow.is_some());
    }
}
