#[derive(Debug, Clone, Copy)]
pub struct HotkeyEvent {
    pub id: i32,
    pub vk: u32,
    pub modifiers: u32,
}
