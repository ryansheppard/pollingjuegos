//! Mirrors the design tokens and component styles in `frontend/src/style.css`
//! so the desktop app reads as the same product as the web app.

use std::sync::LazyLock;

use iced::font::Weight;
use iced::theme::{Mode, Palette};
use iced::widget::{
    button, container, pick_list, progress_bar, rule, scrollable, text, text_input,
};
use iced::{Background, Border, Color, Font, Shadow, Theme, Vector, border, color};

pub const FONTS: [&[u8]; 6] = [
    include_bytes!("../fonts/DMSans-Regular.ttf"),
    include_bytes!("../fonts/DMSans-Medium.ttf"),
    include_bytes!("../fonts/DMSans-SemiBold.ttf"),
    include_bytes!("../fonts/DMSans-Bold.ttf"),
    include_bytes!("../fonts/BarlowCondensed-Bold.ttf"),
    include_bytes!("../fonts/BarlowCondensed-ExtraBold.ttf"),
];

pub const BODY: Font = Font::with_name("DM Sans");
pub const SEMIBOLD: Font = Font {
    weight: Weight::Semibold,
    ..BODY
};
pub const BOLD: Font = Font {
    weight: Weight::Bold,
    ..BODY
};
/// Headings, stats and slot numbers.
pub const DISPLAY: Font = Font {
    weight: Weight::ExtraBold,
    ..Font::with_name("Barlow Condensed")
};
pub const NUMBER: Font = Font {
    weight: Weight::Bold,
    ..DISPLAY
};

/// Matches the web's `button:disabled { opacity: .42 }`.
const DISABLED_ALPHA: f32 = 0.42;

pub struct Tokens {
    pub bg: Color,
    pub surface: Color,
    pub surface_2: Color,
    pub ink: Color,
    pub ink_2: Color,
    pub muted: Color,
    pub subtle: Color,
    pub faint: Color,
    pub line: Color,
    pub line_2: Color,
    pub control_line: Color,
    pub header: Color,
    pub band: Color,
    pub band_line: Color,
    pub on_dark: Color,
    pub on_dark_muted: Color,
    pub accent: Color,
    pub accent_text: Color,
    pub accent_bright: Color,
    pub accent_soft: Color,
    pub accent_soft_ink: Color,
    pub chip: Color,
    pub chip_ink: Color,
    pub tab_chip: Color,
    pub tab_chip_selected: Color,
    pub add_ink: Color,
    pub remove_ink: Color,
    pub track: Color,
    pub tick: Color,
    pub bar: Color,
    pub up: Color,
    pub down: Color,
    pub new: Color,
    pub primary: Color,
    pub primary_hover: Color,
    pub primary_ink: Color,
    pub error_bg: Color,
    pub error_ink: Color,
    pub success_bg: Color,
    pub success_ink: Color,
    pub rank_top_bg: Color,
    pub rank_top_ink: Color,
    pub rank_low_bg: Color,
    pub rank_low_ink: Color,
    pub shadow: Shadow,
}

pub const LIGHT: Tokens = Tokens {
    bg: color!(0xf3f1ec),
    surface: color!(0xffffff),
    surface_2: color!(0xf8f7f3),
    ink: color!(0x17283b),
    ink_2: color!(0x30485f),
    muted: color!(0x5d6977),
    subtle: color!(0x6a7684),
    faint: color!(0xc3cad1),
    line: color!(0xedf0f1),
    line_2: color!(0xe0e2e2),
    control_line: color!(0xd5dbdf),
    header: color!(0x122b46),
    band: color!(0x17324f),
    band_line: color!(0x35506d),
    on_dark: color!(0xffffff),
    on_dark_muted: color!(0xc3cfdb),
    accent: color!(0xc75b2a),
    accent_text: color!(0xac4e24),
    accent_bright: color!(0xf09866),
    accent_soft: color!(0xfbeadf),
    accent_soft_ink: color!(0xa8491e),
    chip: color!(0xedf1f4),
    chip_ink: color!(0x30485f),
    tab_chip: color!(0x2c4a6a),
    tab_chip_selected: color!(0x122b46),
    add_ink: color!(0x122b46),
    remove_ink: color!(0x8a6457),
    track: color!(0xedf0f1),
    tick: color!(0xdde2e5),
    bar: color!(0x234361),
    up: color!(0x1d6b3a),
    down: color!(0xa8491e),
    new: color!(0x1f6f8b),
    primary: color!(0x122b46),
    primary_hover: color!(0x234361),
    primary_ink: color!(0xffffff),
    error_bg: color!(0xfcebe7),
    error_ink: color!(0x982c24),
    success_bg: color!(0xe6f3e9),
    success_ink: color!(0x225a35),
    rank_top_bg: color!(0xe3f1e7),
    rank_top_ink: color!(0x1f5631),
    rank_low_bg: color!(0xfbe8dc),
    rank_low_ink: color!(0x9c431b),
    shadow: Shadow {
        color: color!(0x122b46, 0.035),
        offset: Vector::new(0.0, 6.0),
        blur_radius: 22.0,
    },
};

pub const DARK: Tokens = Tokens {
    bg: color!(0x0e1b2b),
    surface: color!(0x17283b),
    surface_2: color!(0x132335),
    ink: color!(0xebeff3),
    ink_2: color!(0xd2dbe4),
    muted: color!(0xb3bfcb),
    subtle: color!(0xa6b3c0),
    faint: color!(0x4f6378),
    line: color!(0x2a3c50),
    line_2: color!(0x354657),
    control_line: color!(0x425469),
    header: color!(0x0a1522),
    band: color!(0x112236),
    band_line: color!(0x2c435c),
    on_dark: color!(0xffffff),
    on_dark_muted: color!(0xb3c2d1),
    accent: color!(0xf09866),
    accent_text: color!(0xf09866),
    accent_bright: color!(0xf09866),
    accent_soft: color!(0x573a2b),
    accent_soft_ink: color!(0xffd0aa),
    chip: color!(0x29405a),
    chip_ink: color!(0xe0eaf2),
    tab_chip: color!(0x2c4a6a),
    tab_chip_selected: color!(0x29405a),
    add_ink: color!(0xebeff3),
    remove_ink: color!(0xd7a99a),
    track: color!(0x2a3c50),
    tick: color!(0x354657),
    bar: color!(0x7793b0),
    up: color!(0x86d6a0),
    down: color!(0xf5a77e),
    new: color!(0x8fd0e6),
    primary: color!(0xe17e49),
    primary_hover: color!(0xf09866),
    primary_ink: color!(0x101c2a),
    error_bg: color!(0x492d34),
    error_ink: color!(0xffc1b8),
    success_bg: color!(0x234536),
    success_ink: color!(0xbde8c9),
    rank_top_bg: color!(0x234536),
    rank_top_ink: color!(0xbde8c9),
    rank_low_bg: color!(0x3d2c27),
    rank_low_ink: color!(0xf5a77e),
    shadow: Shadow {
        color: Color::TRANSPARENT,
        offset: Vector::ZERO,
        blur_radius: 0.0,
    },
};

fn palette(tokens: &Tokens) -> Palette {
    Palette {
        background: tokens.bg,
        text: tokens.ink,
        primary: tokens.primary,
        success: tokens.up,
        warning: tokens.accent,
        danger: tokens.error_ink,
    }
}

static LIGHT_THEME: LazyLock<Theme> =
    LazyLock::new(|| Theme::custom("PollingJuegos Light", palette(&LIGHT)));
static DARK_THEME: LazyLock<Theme> =
    LazyLock::new(|| Theme::custom("PollingJuegos Dark", palette(&DARK)));

/// Like the web app, follows the system and falls back to light when the
/// system doesn't say.
pub fn theme(mode: Mode) -> Theme {
    match mode {
        Mode::Dark => DARK_THEME.clone(),
        Mode::Light | Mode::None => LIGHT_THEME.clone(),
    }
}

pub fn tokens(theme: &Theme) -> &'static Tokens {
    if theme.extended_palette().is_dark {
        &DARK
    } else {
        &LIGHT
    }
}

fn fade(color: Color, alpha: f32) -> Color {
    Color {
        a: color.a * alpha,
        ..color
    }
}

fn line(color: Color, radius: f32) -> Border {
    Border {
        color,
        width: 1.0,
        radius: radius.into(),
    }
}

// Containers

fn fill(background: Color, text_color: Color) -> container::Style {
    container::Style {
        background: Some(background.into()),
        text_color: Some(text_color),
        ..container::Style::default()
    }
}

pub fn page(theme: &Theme) -> container::Style {
    let t = tokens(theme);
    fill(t.bg, t.ink)
}

pub fn site_header(theme: &Theme) -> container::Style {
    let t = tokens(theme);
    fill(t.header, t.on_dark)
}

pub fn band(theme: &Theme) -> container::Style {
    let t = tokens(theme);
    fill(t.band, t.on_dark)
}

pub fn brand_mark(theme: &Theme) -> container::Style {
    container::Style {
        border: border::rounded(4),
        ..fill(tokens(theme).accent_bright, LIGHT.header)
    }
}

pub fn week_stepper(theme: &Theme) -> container::Style {
    container::Style {
        border: line(tokens(theme).band_line, 6.0),
        ..container::Style::default()
    }
}

pub fn stats(theme: &Theme) -> container::Style {
    container::Style {
        border: line(tokens(theme).band_line, 8.0),
        ..container::Style::default()
    }
}

pub fn tab_chip(selected: bool) -> impl Fn(&Theme) -> container::Style {
    move |theme| {
        let t = tokens(theme);
        let background = if selected {
            t.tab_chip_selected
        } else {
            t.tab_chip
        };
        container::Style {
            border: border::rounded(999),
            ..fill(background, Color::WHITE)
        }
    }
}

pub fn panel(theme: &Theme) -> container::Style {
    let t = tokens(theme);
    container::Style {
        border: line(t.line_2, 10.0),
        shadow: t.shadow,
        ..fill(t.surface, t.ink)
    }
}

pub fn save_bar(theme: &Theme) -> container::Style {
    let t = tokens(theme);
    container::Style {
        border: Border {
            radius: border::bottom(10),
            ..Border::default()
        },
        ..fill(t.surface_2, t.ink)
    }
}

pub fn segmented(theme: &Theme) -> container::Style {
    let t = tokens(theme);
    container::Style {
        border: line(t.control_line, 8.0),
        ..fill(t.surface_2, t.ink)
    }
}

pub fn open_slot(theme: &Theme) -> container::Style {
    let t = tokens(theme);
    container::Style {
        text_color: Some(t.subtle),
        border: Border {
            width: 1.5,
            ..line(t.control_line, 6.0)
        },
        ..container::Style::default()
    }
}

pub fn team_badge(theme: &Theme) -> container::Style {
    let t = tokens(theme);
    container::Style {
        border: border::rounded(4),
        ..fill(t.chip, t.chip_ink)
    }
}

pub fn inset(theme: &Theme) -> container::Style {
    let t = tokens(theme);
    container::Style {
        border: border::rounded(8),
        ..fill(t.surface_2, t.ink)
    }
}

pub fn notice_error(theme: &Theme) -> container::Style {
    let t = tokens(theme);
    container::Style {
        border: border::rounded(8),
        ..fill(t.error_bg, t.error_ink)
    }
}

pub fn notice_success(theme: &Theme) -> container::Style {
    let t = tokens(theme);
    container::Style {
        border: border::rounded(8),
        ..fill(t.success_bg, t.success_ink)
    }
}

pub fn notice_undo(theme: &Theme) -> container::Style {
    let t = tokens(theme);
    container::Style {
        border: border::rounded(8),
        ..fill(t.surface_2, t.ink_2)
    }
}

pub fn scrim(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.5).into()),
        ..container::Style::default()
    }
}

/// The colors a crowd/stat badge takes for each tone; the web's `.crowd-chip`.
#[derive(Clone, Copy)]
pub enum Chip {
    Plain,
    Unranked,
    Top,
    Mid,
    Low,
}

pub fn chip(kind: Chip) -> impl Fn(&Theme) -> container::Style {
    move |theme| {
        let t = tokens(theme);
        let (background, ink) = match kind {
            Chip::Plain => (t.chip, t.chip_ink),
            Chip::Unranked => (t.surface_2, t.subtle),
            Chip::Top => (t.rank_top_bg, t.rank_top_ink),
            Chip::Mid => (Color::TRANSPARENT, t.ink_2),
            Chip::Low => (t.rank_low_bg, t.rank_low_ink),
        };
        container::Style {
            border: border::rounded(4),
            ..fill(background, ink)
        }
    }
}

pub fn swatch(kind: Chip) -> impl Fn(&Theme) -> container::Style {
    move |theme| {
        let t = tokens(theme);
        let (background, edge) = match kind {
            Chip::Top => (t.rank_top_bg, t.rank_top_ink),
            Chip::Low => (t.rank_low_bg, t.rank_low_ink),
            Chip::Plain | Chip::Unranked | Chip::Mid => (Color::TRANSPARENT, t.control_line),
        };
        container::Style {
            background: Some(background.into()),
            border: line(edge, 3.0),
            ..container::Style::default()
        }
    }
}

// Rules

pub fn divider(theme: &Theme) -> rule::Style {
    rule::Style {
        color: tokens(theme).line,
        radius: 0.into(),
        fill_mode: rule::FillMode::Full,
        snap: true,
    }
}

pub fn band_divider(theme: &Theme) -> rule::Style {
    rule::Style {
        color: tokens(theme).band_line,
        ..divider(theme)
    }
}

// Text

fn color_text(color: Color) -> text::Style {
    text::Style { color: Some(color) }
}

pub fn ink_2(theme: &Theme) -> text::Style {
    color_text(tokens(theme).ink_2)
}

pub fn muted(theme: &Theme) -> text::Style {
    color_text(tokens(theme).muted)
}

pub fn subtle(theme: &Theme) -> text::Style {
    color_text(tokens(theme).subtle)
}

pub fn faint(theme: &Theme) -> text::Style {
    color_text(tokens(theme).faint)
}

pub fn accent_text(theme: &Theme) -> text::Style {
    color_text(tokens(theme).accent_text)
}

pub fn accent_bright(theme: &Theme) -> text::Style {
    color_text(tokens(theme).accent_bright)
}

pub fn on_dark(theme: &Theme) -> text::Style {
    color_text(tokens(theme).on_dark)
}

pub fn on_dark_muted(theme: &Theme) -> text::Style {
    color_text(tokens(theme).on_dark_muted)
}

pub fn up(theme: &Theme) -> text::Style {
    color_text(tokens(theme).up)
}

pub fn down(theme: &Theme) -> text::Style {
    color_text(tokens(theme).down)
}

pub fn new(theme: &Theme) -> text::Style {
    color_text(tokens(theme).new)
}

// Buttons

fn button_base(background: Color, text_color: Color, border: Border) -> button::Style {
    button::Style {
        background: Some(background.into()),
        text_color,
        border,
        ..button::Style::default()
    }
}

fn disabled(style: button::Style) -> button::Style {
    button::Style {
        background: style.background.map(|background| match background {
            Background::Color(color) => fade(color, DISABLED_ALPHA).into(),
            other => other,
        }),
        text_color: fade(style.text_color, DISABLED_ALPHA),
        border: Border {
            color: fade(style.border.color, DISABLED_ALPHA),
            ..style.border
        },
        ..style
    }
}

fn hovering(status: button::Status) -> bool {
    matches!(status, button::Status::Hovered | button::Status::Pressed)
}

/// `.save-button` and other primary actions.
pub fn primary(theme: &Theme, status: button::Status) -> button::Style {
    let t = tokens(theme);
    let background = if hovering(status) {
        t.primary_hover
    } else {
        t.primary
    };
    let style = button_base(background, t.primary_ink, border::rounded(6));
    match status {
        button::Status::Disabled => disabled(style),
        _ => style,
    }
}

/// `.secondary-button`.
pub fn secondary(theme: &Theme, status: button::Status) -> button::Style {
    let t = tokens(theme);
    match status {
        button::Status::Hovered | button::Status::Pressed => {
            button_base(t.surface, t.accent_text, line(t.accent, 6.0))
        }
        button::Status::Active => button_base(t.surface, t.ink, line(t.control_line, 6.0)),
        button::Status::Disabled => {
            disabled(button_base(t.surface, t.ink, line(t.control_line, 6.0)))
        }
    }
}

/// `.add-button`: fills in on hover since it's the pool's main action.
pub fn add(theme: &Theme, status: button::Status) -> button::Style {
    let t = tokens(theme);
    match status {
        button::Status::Hovered | button::Status::Pressed => {
            button_base(t.primary, t.primary_ink, line(t.primary, 6.0))
        }
        button::Status::Active => button_base(t.surface, t.add_ink, line(t.control_line, 6.0)),
        button::Status::Disabled => {
            disabled(button_base(t.surface, t.add_ink, line(t.control_line, 6.0)))
        }
    }
}

/// `.icon-button`.
pub fn icon(theme: &Theme, status: button::Status) -> button::Style {
    let t = tokens(theme);
    let background = if hovering(status) { t.chip } else { t.surface };
    let style = button_base(background, t.ink_2, line(t.line_2, 6.0));
    match status {
        button::Status::Disabled => disabled(style),
        _ => style,
    }
}

/// `.remove-button`.
pub fn remove(theme: &Theme, status: button::Status) -> button::Style {
    let t = tokens(theme);
    match status {
        button::Status::Hovered | button::Status::Pressed => {
            button_base(t.accent_soft, t.accent_soft_ink, border::rounded(6))
        }
        button::Status::Active => button_base(Color::TRANSPARENT, t.remove_ink, border::rounded(6)),
        button::Status::Disabled => disabled(button_base(
            Color::TRANSPARENT,
            t.remove_ink,
            border::rounded(6),
        )),
    }
}

/// One option in a `.lens-switch`-style segmented control.
pub fn segment(selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        let t = tokens(theme);
        if selected {
            return button_base(t.primary, t.primary_ink, border::rounded(6));
        }
        let background = if hovering(status) {
            t.chip
        } else {
            Color::TRANSPARENT
        };
        let style = button_base(background, t.ink_2, border::rounded(6));
        match status {
            button::Status::Disabled => disabled(style),
            _ => style,
        }
    }
}

/// `.tab`: the selected tab takes the page color so it joins the content below.
pub fn tab(selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        let t = tokens(theme);
        let radius = border::top(8);
        let style = if selected {
            button_base(
                t.bg,
                t.ink,
                Border {
                    radius,
                    ..Border::default()
                },
            )
        } else if hovering(status) {
            button_base(
                Color::from_rgba(1.0, 1.0, 1.0, 0.06),
                t.on_dark,
                Border {
                    radius,
                    ..Border::default()
                },
            )
        } else {
            button_base(
                Color::TRANSPARENT,
                t.on_dark_muted,
                Border {
                    radius,
                    ..Border::default()
                },
            )
        };
        match status {
            button::Status::Disabled => disabled(style),
            _ => style,
        }
    }
}

/// `.step-button` in the header's week stepper.
pub fn header_step(theme: &Theme, status: button::Status) -> button::Style {
    let t = tokens(theme);
    let background = if hovering(status) {
        Color::from_rgba(1.0, 1.0, 1.0, 0.12)
    } else {
        Color::TRANSPARENT
    };
    let style = button_base(background, t.on_dark, border::rounded(4));
    match status {
        button::Status::Disabled => disabled(style),
        _ => style,
    }
}

/// `.sign-out`: a bare text link on the header.
pub fn header_link(theme: &Theme, status: button::Status) -> button::Style {
    let t = tokens(theme);
    let ink = if hovering(status) {
        t.accent_bright
    } else {
        t.on_dark_muted
    };
    button_base(Color::TRANSPARENT, ink, Border::default())
}

// Inputs

pub fn header_pick_list(theme: &Theme, status: pick_list::Status) -> pick_list::Style {
    let t = tokens(theme);
    let background = match status {
        pick_list::Status::Hovered | pick_list::Status::Opened { .. } => {
            Color::from_rgba(1.0, 1.0, 1.0, 0.12)
        }
        pick_list::Status::Active => Color::TRANSPARENT,
    };
    pick_list::Style {
        text_color: t.on_dark,
        placeholder_color: t.on_dark_muted,
        handle_color: t.on_dark_muted,
        background: background.into(),
        border: border::rounded(4),
    }
}

pub fn menu(theme: &Theme) -> iced::overlay::menu::Style {
    let t = tokens(theme);
    iced::overlay::menu::Style {
        background: t.surface.into(),
        border: line(t.line_2, 6.0),
        text_color: t.ink,
        selected_text_color: t.primary_ink,
        selected_background: t.primary.into(),
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.15),
            offset: Vector::new(0.0, 4.0),
            blur_radius: 16.0,
        },
    }
}

pub fn search(theme: &Theme, status: text_input::Status) -> text_input::Style {
    let t = tokens(theme);
    let edge = match status {
        text_input::Status::Focused { .. } => t.accent,
        text_input::Status::Hovered => t.subtle,
        text_input::Status::Active | text_input::Status::Disabled => t.control_line,
    };
    text_input::Style {
        background: t.surface.into(),
        border: line(edge, 6.0),
        icon: t.subtle,
        placeholder: t.subtle,
        value: t.ink,
        selection: fade(t.accent, 0.3),
    }
}

// Bars

pub fn progress(theme: &Theme) -> progress_bar::Style {
    let t = tokens(theme);
    progress_bar::Style {
        background: t.tick.into(),
        bar: t.accent.into(),
        border: border::rounded(3),
    }
}

pub fn score(leader: bool) -> impl Fn(&Theme) -> progress_bar::Style {
    move |theme| {
        let t = tokens(theme);
        progress_bar::Style {
            background: t.track.into(),
            bar: if leader { t.accent } else { t.bar }.into(),
            border: border::rounded(2),
        }
    }
}

// Scrollbars

/// A thin bar inset from the panel edge so it doesn't sit on the rounded border.
pub fn scrollbar() -> scrollable::Direction {
    scrollable::Direction::Vertical(
        scrollable::Scrollbar::new()
            .width(6)
            .scroller_width(6)
            .margin(4),
    )
}

pub fn scroll(theme: &Theme, status: scrollable::Status) -> scrollable::Style {
    let t = tokens(theme);
    let active = matches!(
        status,
        scrollable::Status::Hovered {
            is_vertical_scrollbar_hovered: true,
            ..
        } | scrollable::Status::Dragged {
            is_vertical_scrollbar_dragged: true,
            ..
        }
    );
    let rail = scrollable::Rail {
        background: None,
        border: Border::default(),
        scroller: scrollable::Scroller {
            background: if active { t.subtle } else { t.control_line }.into(),
            border: border::rounded(3),
        },
    };
    scrollable::Style {
        vertical_rail: rail,
        horizontal_rail: rail,
        gap: None,
        ..scrollable::default(theme, status)
    }
}
