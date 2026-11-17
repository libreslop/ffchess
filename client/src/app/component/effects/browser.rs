//! Browser-facing component effects.

use crate::app::browser::PageMetadata;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{Element, HtmlInputElement};
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

/// Id of the main menu the server prerenders into the page.
const BOOT_SCREEN_ID: &str = "boot-screen";

/// Selector of the name input, in both the prerendered menu and the app's own.
const NAME_INPUT_SELECTOR: &str = "input[name=player_name]";

/// Removes the prerendered main menu once the app shows a screen of its own, taking over the
/// name typed into it, and the focus when it had it.
#[hook]
pub fn use_boot_screen_effect(app_ready: bool, player_name: UseStateHandle<String>) {
    use_effect_with(app_ready, move |&app_ready| {
        let document = gloo_utils::document();
        let Some(boot_screen) = document
            .get_element_by_id(BOOT_SCREEN_ID)
            .filter(|_| app_ready)
        else {
            return;
        };
        let boot_input = as_input(boot_screen.query_selector(NAME_INPUT_SELECTOR));
        let had_focus = boot_input
            .as_ref()
            .is_some_and(|input| document.active_element().as_ref() == Some(input.unchecked_ref()));
        boot_screen.remove();
        if let Some(name) = boot_input
            .map(|input| input.value())
            .filter(|name| !name.is_empty())
        {
            player_name.set(name);
        }
        // With the prerendered menu gone, the only name input left is the app's.
        if had_focus && let Some(input) = as_input(document.query_selector(NAME_INPUT_SELECTOR)) {
            let _ = input.focus();
        }
    });
}

/// Returns the element a `query_selector` call found, if it is an input.
fn as_input(found: Result<Option<Element>, JsValue>) -> Option<HtmlInputElement> {
    found.ok().flatten()?.dyn_into().ok()
}
