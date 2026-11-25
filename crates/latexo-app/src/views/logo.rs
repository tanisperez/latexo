//! The app icon (`assets/latexo.svg`, from `packaging/icons/latexo.svg`): a pulse line on a
//! dark rounded square, in the side bar. An image, not a gpui `svg`, because gpui paints an `svg`
//! in one color and the icon has several.

use std::sync::{Arc, LazyLock};

use gpui_kit::*;

/// The icon, parsed once and shared, so gpui's image cache keeps one rasterization per size.
static ICON: LazyLock<Arc<Image>> = LazyLock::new(|| {
    Arc::new(Image::from_bytes(
        ImageFormat::Svg,
        include_bytes!("../../assets/latexo.svg").to_vec(),
    ))
});

/// The icon, `size` logical pixels square.
pub fn logo(size: f32) -> Img {
    img(ICON.clone()).flex_none().size(px(size))
}
