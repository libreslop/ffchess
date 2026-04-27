//! Browser favicon helpers.

use crate::canvas::color::piece_icon_colors;

const KING_FAVICON_TEMPLATE: &str = include_str!("../../../assets/pieces/king.svg");

/// Updates the browser tab favicon to a king icon tinted with the provided team color.
pub fn set_team_favicon(team_color: &str) {
    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        return;
    };

    let selector = "link[data-ffchess-favicon='true']";
    let favicon = document
        .query_selector(selector)
        .ok()
        .flatten()
        .or_else(|| {
            let link = document.create_element("link").ok()?;
            link.set_attribute("rel", "icon").ok()?;
            link.set_attribute("type", "image/svg+xml").ok()?;
            link.set_attribute("data-ffchess-favicon", "true").ok()?;
            let head = document.head()?;
            let _ = head.append_child(&link);
            Some(link)
        });

    let Some(favicon) = favicon else {
        return;
    };

    let colors = piece_icon_colors(team_color);
    let tinted_svg = KING_FAVICON_TEMPLATE
        .replace("${primary}", &colors.primary)
        .replace("${secondary}", &colors.secondary)
        .replace("${tertiary}", &colors.tertiary)
        .replace("shape-rendering=\"crispEdges\"", "shape-rendering=\"auto\"");
    let encoded = js_sys::encode_uri_component(&tinted_svg)
        .as_string()
        .unwrap_or_default();
    let href = format!("data:image/svg+xml;utf8,{encoded}");
    let _ = favicon.set_attribute("href", &href);
}
