use crate::models::hotkey::KeyBoardShortcut;

#[derive(Debug, Clone, Hash)]

pub struct Action {
    // This struct represent the action a user takes and all necessary context required to take this action
    context: String,
    source: String,
    name: String,
    action: KeyBoardShortcut,
}

type ActionId = u32; // uniquely identifies what the user is trying to do, ie paste, copy, new tab
                     // Not sure if u32 or string or some special struct is better
                     // Open to change
