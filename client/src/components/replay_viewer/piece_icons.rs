//! Piece icons of the move history, tinted in the colors of the players owning them.

use crate::canvas::color::{NPC_PIECE_COLOR, tinted_piece_data_url};
use common::rules::Catalog;
use common::types::{ColorHex, PieceTypeId};
use futures_util::future::join_all;
use gloo_net::http::Request;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use yew::AttrValue;

/// Icons of the piece types of a replay, tinted on first use.
pub(super) struct PieceIcons {
    /// SVG templates by piece type; types whose template could not be loaded are missing.
    templates: HashMap<PieceTypeId, String>,
    /// Data URLs of the icons tinted so far, by piece type and color.
    tinted: RefCell<HashMap<(PieceTypeId, String), AttrValue>>,
}

impl PieceIcons {
    /// Fetches the SVG template of every piece type in `catalog`.
    pub(super) async fn fetch(catalog: &Catalog) -> Self {
        let fetches = catalog
            .pieces
            .iter()
            .filter(|(_, config)| !config.svg_path.trim().is_empty())
            .map(|(piece_type, config)| async move {
                let response = Request::get(&format!("/assets/pieces/{}", config.svg_path))
                    .send()
                    .await
                    .ok()
                    .filter(|response| response.ok())?;
                Some((piece_type.clone(), response.text().await.ok()?))
            });
        Self {
            templates: join_all(fetches).await.into_iter().flatten().collect(),
            tinted: RefCell::default(),
        }
    }

    /// Returns the icon of `piece_type` in the player color `color`, or in the color of
    /// pieces nobody owns, or `None` when its template could not be loaded.
    pub(super) fn icon(
        &self,
        piece_type: &PieceTypeId,
        color: Option<&ColorHex>,
    ) -> Option<AttrValue> {
        let template = self.templates.get(piece_type)?;
        let color = color.map_or(NPC_PIECE_COLOR, |color| color.as_ref());
        let icon = self
            .tinted
            .borrow_mut()
            .entry((piece_type.clone(), color.to_string()))
            .or_insert_with(|| tinted_piece_data_url(template, color).into())
            .clone();
        Some(icon)
    }
}

/// Shared handle to piece icons, compared by identity so re-renders stay cheap.
#[derive(Clone)]
pub(super) struct SharedIcons(pub(super) Rc<PieceIcons>);

impl PartialEq for SharedIcons {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}
