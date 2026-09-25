pub(super) const INK: Color = Color::rgb(26, 28, 44);
pub(super) const NAVY: Color = Color::rgb(41, 54, 111);
pub(super) const ROYAL: Color = Color::rgb(59, 93, 201);
pub(super) const SKY: Color = Color::rgb(65, 166, 246);
pub(super) const PARCH: Color = Color::rgb(244, 244, 244);
pub(super) const MIST: Color = Color::rgb(148, 176, 194);
pub(super) const GOLD: Color = Color::rgb(255, 205, 117);
pub(super) const GREEN: Color = Color::rgb(56, 183, 100);
pub(super) const RED: Color = Color::rgb(225, 75, 95);
pub(super) const PLUM: Color = Color::rgb(93, 39, 93);
pub(super) const CRAB: Color = Color::rgb(206, 142, 107);
pub(super) const VOID: Color = Color::rgb(7, 10, 24);
pub(super) const INDIGO: Color = Color::rgb(22, 29, 66);
pub(super) const CYAN: Color = Color::rgb(67, 224, 244);
pub(super) const CYAN_DIM: Color = Color::rgb(47, 146, 174);
pub(super) const AMBER: Color = Color::rgb(247, 183, 72);
pub(super) const VIOLET: Color = Color::rgb(105, 58, 151);
pub(super) const MAGENTA: Color = Color::rgb(213, 91, 151);
pub(super) const ASH: Color = Color::rgb(68, 78, 105);

#[derive(Clone, Copy)]
pub(super) struct Color(pub(super) u8, pub(super) u8, pub(super) u8);

impl Color {
    pub(super) const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self(r, g, b)
    }
}
