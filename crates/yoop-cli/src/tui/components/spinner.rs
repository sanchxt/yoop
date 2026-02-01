//! Spinner component for loading states.
//!
//! Displays an animated spinner with optional text for async operations.

use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

/// Spinner animation frames (Braille pattern)
const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Alternate spinner (dots)
const SPINNER_DOTS: &[&str] = &["⣾", "⣽", "⣻", "⢿", "⡿", "⣟", "⣯", "⣷"];

/// Line spinner
const SPINNER_LINE: &[&str] = &["-", "\\", "|", "/"];

/// Spinner style variants
#[derive(Debug, Clone, Copy, Default)]
pub enum SpinnerStyle {
    /// Braille pattern spinner (default)
    #[default]
    Braille,
    /// Dots pattern spinner
    Dots,
    /// Simple line spinner
    Line,
}

impl SpinnerStyle {
    /// Get the animation frames for this style.
    pub const fn frames(&self) -> &'static [&'static str] {
        match self {
            SpinnerStyle::Braille => SPINNER_FRAMES,
            SpinnerStyle::Dots => SPINNER_DOTS,
            SpinnerStyle::Line => SPINNER_LINE,
        }
    }

    /// Get the number of frames in the animation.
    pub const fn frame_count(&self) -> usize {
        match self {
            SpinnerStyle::Braille => SPINNER_FRAMES.len(),
            SpinnerStyle::Dots => SPINNER_DOTS.len(),
            SpinnerStyle::Line => SPINNER_LINE.len(),
        }
    }
}

/// Spinner state for tracking animation.
#[derive(Debug, Clone, Default)]
pub struct SpinnerState {
    /// Current frame index
    frame: usize,
    /// Tick counter for animation timing
    tick: u64,
}

impl SpinnerState {
    /// Create a new spinner state.
    pub const fn new() -> Self {
        Self { frame: 0, tick: 0 }
    }

    /// Advance the spinner animation.
    ///
    /// Call this on each tick (typically every 100ms).
    pub fn tick(&mut self, style: SpinnerStyle) {
        self.tick = self.tick.wrapping_add(1);
        if self.tick % 2 == 0 {
            self.frame = (self.frame + 1) % style.frame_count();
        }
    }

    /// Get the current frame character.
    pub fn current_frame(&self, style: SpinnerStyle) -> &'static str {
        style.frames()[self.frame % style.frame_count()]
    }

    /// Reset the spinner state.
    pub fn reset(&mut self) {
        self.frame = 0;
        self.tick = 0;
    }
}

/// Spinner widget for rendering loading states.
pub struct Spinner<'a> {
    /// Text to display alongside spinner
    text: &'a str,
    /// Spinner animation style
    style: SpinnerStyle,
    /// Text style
    text_style: Style,
    /// Spinner style (color)
    spinner_style: Style,
}

impl<'a> Spinner<'a> {
    /// Create a new spinner with the given text.
    pub fn new(text: &'a str) -> Self {
        Self {
            text,
            style: SpinnerStyle::default(),
            text_style: Style::default(),
            spinner_style: Style::default(),
        }
    }

    /// Set the spinner animation style.
    #[must_use]
    pub const fn animation_style(mut self, style: SpinnerStyle) -> Self {
        self.style = style;
        self
    }

    /// Set the text style.
    #[must_use]
    pub const fn text_style(mut self, style: Style) -> Self {
        self.text_style = style;
        self
    }

    /// Set the spinner character style.
    #[must_use]
    pub const fn spinner_style(mut self, style: Style) -> Self {
        self.spinner_style = style;
        self
    }

    /// Render the spinner.
    pub fn render(&self, frame: &mut Frame, area: Rect, state: &SpinnerState) {
        let spinner_char = state.current_frame(self.style);
        let line = Line::from(vec![
            Span::styled(spinner_char, self.spinner_style),
            Span::raw(" "),
            Span::styled(self.text, self.text_style),
        ]);
        let paragraph = Paragraph::new(line);
        frame.render_widget(paragraph, area);
    }
}

/// Loading indicator with message and optional progress.
pub struct LoadingIndicator<'a> {
    /// Loading message
    message: &'a str,
    /// Optional secondary message (e.g., "Connecting to peer...")
    detail: Option<&'a str>,
    /// Spinner style
    style: SpinnerStyle,
    /// Theme colors
    accent_style: Style,
    /// Text style
    text_style: Style,
    /// Muted text style
    muted_style: Style,
}

impl<'a> LoadingIndicator<'a> {
    /// Create a new loading indicator.
    pub fn new(message: &'a str) -> Self {
        Self {
            message,
            detail: None,
            style: SpinnerStyle::default(),
            accent_style: Style::default(),
            text_style: Style::default(),
            muted_style: Style::default(),
        }
    }

    /// Set the detail message.
    #[must_use]
    pub const fn detail(mut self, detail: &'a str) -> Self {
        self.detail = Some(detail);
        self
    }

    /// Set the spinner animation style.
    #[must_use]
    pub const fn animation_style(mut self, style: SpinnerStyle) -> Self {
        self.style = style;
        self
    }

    /// Set the accent style (for spinner).
    #[must_use]
    pub const fn accent_style(mut self, style: Style) -> Self {
        self.accent_style = style;
        self
    }

    /// Set the text style.
    #[must_use]
    pub const fn text_style(mut self, style: Style) -> Self {
        self.text_style = style;
        self
    }

    /// Set the muted style (for detail text).
    #[must_use]
    pub const fn muted_style(mut self, style: Style) -> Self {
        self.muted_style = style;
        self
    }

    /// Render the loading indicator.
    pub fn render(&self, frame: &mut Frame, area: Rect, state: &SpinnerState) {
        if area.height == 0 {
            return;
        }

        let spinner_char = state.current_frame(self.style);

        let mut lines = vec![Line::from(vec![
            Span::styled(spinner_char, self.accent_style),
            Span::raw(" "),
            Span::styled(self.message, self.text_style),
        ])];

        if let Some(detail) = self.detail {
            if area.height > 1 {
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(detail, self.muted_style),
                ]));
            }
        }

        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spinner_state_tick() {
        let mut state = SpinnerState::new();
        assert_eq!(state.frame, 0);

        state.tick(SpinnerStyle::Braille);
        assert_eq!(state.frame, 0);

        state.tick(SpinnerStyle::Braille);
        assert_eq!(state.frame, 1);
    }

    #[test]
    fn test_spinner_frame_wrap() {
        let mut state = SpinnerState::new();
        let style = SpinnerStyle::Line;

        for _ in 0..20 {
            state.tick(style);
        }

        assert!(state.frame < style.frame_count());
    }

    #[test]
    fn test_spinner_reset() {
        let mut state = SpinnerState::new();
        state.tick(SpinnerStyle::Braille);
        state.tick(SpinnerStyle::Braille);
        state.tick(SpinnerStyle::Braille);
        state.tick(SpinnerStyle::Braille);

        state.reset();
        assert_eq!(state.frame, 0);
        assert_eq!(state.tick, 0);
    }
}
