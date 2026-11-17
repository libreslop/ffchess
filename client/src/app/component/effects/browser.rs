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
/// name typed into it, and the focus and selection when it had the focus.
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
        let name = boot_input.as_ref().map(HtmlInputElement::value);
        let selection = boot_input
            .filter(|input| document.active_element().as_ref() == Some(input.unchecked_ref()))
            .and_then(|input| InputSelection::of(&input));
        boot_screen.remove();

        // With the prerendered menu gone, the only name input left is the app's.
        let app_input = as_input(document.query_selector(NAME_INPUT_SELECTOR));
        if let Some(name) = name.filter(|name| !name.is_empty()) {
            // Changing an input's value moves its caret to the end. Filling the name in now,
            // before the selection is restored, leaves the app's render nothing to change.
            if let Some(app_input) = &app_input {
                app_input.set_value(&name);
            }
            player_name.set(name);
        }
        if let (Some(app_input), Some(selection)) = (app_input, selection) {
            let _ = app_input.focus();
            selection.apply_to(&app_input);
        }
    });
}

/// The selected text of an input, or its caret when nothing is selected.
struct InputSelection {
    start: u32,
    end: u32,
    /// "forward", "backward", or "none", the end the selection grows from.
    direction: String,
}

impl InputSelection {
    /// Returns the selection of `input`, if it has one.
    fn of(input: &HtmlInputElement) -> Option<Self> {
        Some(Self {
            start: input.selection_start().ok()??,
            end: input.selection_end().ok()??,
            direction: input.selection_direction().ok()??,
        })
    }

    /// Selects the same range in `input`.
    fn apply_to(&self, input: &HtmlInputElement) {
        let _ = input.set_selection_range_with_direction(self.start, self.end, &self.direction);
    }
}

/// Returns the element a `query_selector` call found, if it is an input.
fn as_input(found: Result<Option<Element>, JsValue>) -> Option<HtmlInputElement> {
    found.ok().flatten()?.dyn_into().ok()
}
