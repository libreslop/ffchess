//! Browser-facing component effects.

use crate::app::browser::PageMetadata;
use yew::hook;
use yew::prelude::*;

/// Keeps the browser title and favicon in sync with the selected mode and local team.
#[hook]
pub fn use_page_metadata_effect(page_metadata: PageMetadata) {
    use_effect_with(page_metadata, move |page_metadata| {
        page_metadata.apply();
        || ()
    });
}
