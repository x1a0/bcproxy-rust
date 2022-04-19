mod color256;
mod true_color;

pub use self::color256::colorize_content;

pub enum FgBg {
    FG,
    BG,
}

impl FgBg {
    pub fn value(&self) -> &str {
        match self {
            FgBg::FG => "38",
            FgBg::BG => "48",
        }
    }
}
