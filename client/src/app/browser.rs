//! Browser page metadata helpers.

use crate::canvas::color::NPC_PIECE_COLOR;
use crate::canvas::color::piece_icon_colors;

const APP_TITLE: &str = "FFChess";
const KING_FAVICON_TEMPLATE: &str = include_str!("../../../assets/pieces/king.svg");

/// Browser metadata derived from the selected mode and the local player's team.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PageMetadata {
    mode_name: Option<String>,
    team_color: Option<String>,
}

impl PageMetadata {
    /// Builds page metadata from the currently selected mode and local team color.
    pub fn new(mode_name: Option<String>, team_color: Option<String>) -> Self {
        Self {
            mode_name: normalize_text(mode_name),
            team_color: normalize_text(team_color),
        }
    }

    /// Applies the metadata to the browser document.
    pub fn apply(&self) {
        set_document_title(&self.title());
        set_team_favicon(self.favicon_color());
    }

    fn title(&self) -> String {
        self.mode_name
            .as_deref()
            .map(|mode_name| format!("{mode_name} - {APP_TITLE}"))
            .unwrap_or_else(|| APP_TITLE.to_string())
    }

    fn favicon_color(&self) -> &str {
        self.team_color.as_deref().unwrap_or(NPC_PIECE_COLOR)
    }
}

fn normalize_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

fn set_document_title(title: &str) {
    if let Some(document) = web_sys::window().and_then(|window| window.document()) {
        document.set_title(title);
    }
}

fn set_team_favicon(team_color: &str) {
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

    let _ = favicon.set_attribute("href", &king_favicon_data_url(team_color));
}

fn king_favicon_data_url(team_color: &str) -> String {
    let colors = piece_icon_colors(team_color);
    let tinted_svg = KING_FAVICON_TEMPLATE
        .replace("${primary}", &colors.primary)
        .replace("${secondary}", &colors.secondary)
        .replace("${tertiary}", &colors.tertiary)
        .replace("shape-rendering=\"crispEdges\"", "shape-rendering=\"auto\"");
    let encoded = js_sys::encode_uri_component(&tinted_svg)
        .as_string()
        .unwrap_or_default();
    format!("data:image/svg+xml;utf8,{encoded}")
}
