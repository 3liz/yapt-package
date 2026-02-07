use clap::builder::styling::{Style, Color, AnsiColor};

pub fn info_style() -> Style {
    Style::new().fg_color(Some(Color::Ansi(AnsiColor::Yellow)))
}

macro_rules! info {
    ($($arg:tt)*) => {
        let st = crate::notice::info_style();
        eprint!("{st}");
        eprint!($($arg)*);
        eprintln!("{st:#}");
    };
}

pub(crate) use info;
