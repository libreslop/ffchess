use rand::seq::SliceRandom;
use uuid::Uuid;

pub fn get_stored_name() -> String {
    let window = web_sys::window().unwrap();
    if let Ok(Some(storage)) = window.local_storage() {
        return storage
            .get_item("ffchess_name")
            .unwrap_or_default()
            .unwrap_or_default();
    }
    String::new()
}

pub fn set_stored_name(name: &str) {
    let window = web_sys::window().unwrap();
    if let Ok(Some(storage)) = window.local_storage() {
        let _ = storage.set_item("ffchess_name", name);
    }
}

pub fn get_stored_id() -> Option<Uuid> {
    let window = web_sys::window().unwrap();
    if let Ok(Some(storage)) = window.local_storage() {
        return storage
            .get_item("ffchess_player_id")
            .unwrap_or_default()
            .and_then(|s| Uuid::parse_str(&s).ok());
    }
    None
}

pub fn set_stored_id(id: Uuid) {
    let window = web_sys::window().unwrap();
    if let Ok(Some(storage)) = window.local_storage() {
        let _ = storage.set_item("ffchess_player_id", &id.to_string());
    }
}

pub fn get_death_timestamp() -> i64 {
    let window = web_sys::window().unwrap();
    if let Ok(Some(storage)) = window.local_storage() {
        return storage
            .get_item("ffchess_death_ts")
            .unwrap_or_default()
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0);
    }
    0
}

pub fn set_death_timestamp(ts: i64) {
    let window = web_sys::window().unwrap();
    if let Ok(Some(storage)) = window.local_storage() {
        let _ = storage.set_item("ffchess_death_ts", &ts.to_string());
    }
}

pub fn generate_random_name() -> String {
    let adjectives = [
        "Swift", "Brave", "Silent", "Iron", "Gold", "Shadow", "Grand", "Quick", "Old", "New",
        "Wild", "Calm", "Crimson", "Azure", "Sly", "Mighty", "Ancient", "Fierce", "Noble",
        "Ethereal", "Frosty", "Fiery", "Stormy", "Golden", "Silver", "Hidden", "Lone", "vibrant",
        "Dark", "Bright", "Steady", "Fallen",
    ];
    let nouns = [
        "Knight", "King", "Rook", "Bishop", "Pawn", "Queen", "Warrior", "Shadow", "Storm", "Frost",
        "Flame", "Blade", "Guard", "Seeker", "Warden", "Herald", "Slayer", "Spirit", "Ghost",
        "Titan", "Wolf", "Raven", "Dragon", "Phoenix", "Sentinel", "Oracle", "Monarch", "Paladin",
        "Ranger", "Saber", "Shield", "Fang",
    ];
    let mut rng = rand::thread_rng();
    let adj = adjectives.choose(&mut rng).unwrap();
    let mut noun = nouns.choose(&mut rng).unwrap();
    while noun == adj {
        noun = nouns.choose(&mut rng).unwrap();
    }
    format!("{} {}", adj, noun)
}

pub fn is_mobile() -> bool {
    let window = web_sys::window().expect("no global `window` exists");
    let navigator = window.navigator();
    let user_agent = navigator.user_agent().unwrap_or_default().to_lowercase();
    
    user_agent.contains("mobi") || 
    user_agent.contains("android") || 
    user_agent.contains("iphone") ||
    user_agent.contains("ipad")
}

pub fn request_fullscreen() {
    let window = web_sys::window().expect("no global `window` exists");
    let document = window.document().expect("should have a document on window");
    let element = document.document_element().expect("should have a document element");
    
    let _ = element.request_fullscreen();
}
