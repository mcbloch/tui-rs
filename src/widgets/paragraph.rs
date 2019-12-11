use either::Either;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::buffer::Buffer;
use crate::layout::{Alignment, Rect, ScrollFrom};
use crate::style::Style;
use crate::widgets::reflow::{LineComposer, LineTruncator, Styled, WordWrapper};
use crate::widgets::{Block, Text, Widget};

fn get_line_offset(line_width: u16, text_area_width: u16, alignment: Alignment) -> u16 {
    match alignment {
        Alignment::Center => (text_area_width / 2).saturating_sub(line_width / 2),
        Alignment::Right => text_area_width.saturating_sub(line_width),
        Alignment::Left => 0,
    }
}

fn get_line_vertical_pos(
    text_area: Rect,
    line_idx: u16,
    scroll_offset: u16,
    scroll_from: ScrollFrom,
) -> u16 {
    match scroll_from {
        ScrollFrom::Top => text_area.top() + line_idx - scroll_offset,
        ScrollFrom::Bottom => text_area.bottom() + scroll_offset - (line_idx + 1),
    }
}

/// A widget to display some text.
///
/// # Examples
///
/// ```
/// # use tui::widgets::{Block, Borders, Paragraph, Text};
/// # use tui::style::{Style, Color};
/// # use tui::layout::{Alignment};
/// # fn main() {
/// let text = [
///     Text::raw("First line\n"),
///     Text::styled("Second line\n", Style::default().fg(Color::Red))
/// ];
/// Paragraph::new(text.iter())
///     .block(Block::default().title("Paragraph").borders(Borders::ALL))
///     .style(Style::default().fg(Color::White).bg(Color::Black))
///     .alignment(Alignment::Center)
///     .wrap(true);
/// # }
/// ```
pub struct Paragraph<'a, 't, T>
where
    T: Iterator<Item = &'t Text<'t>>,
{
    /// A block to wrap the widget in
    block: Option<Block<'a>>,
    /// Widget style
    style: Style,
    /// Wrap the text or not
    wrapping: bool,
    /// The text to display
    text: T,
    /// Should we parse the text for embedded commands
    raw: bool,
    /// Scroll offset (in number of lines)
    scroll: u16,
    /// Indicates if scroll offset starts from top or bottom of content
    scroll_from: ScrollFrom,
    /// Aligenment of the text
    alignment: Alignment,
}

impl<'a, 't, T> Paragraph<'a, 't, T>
where
    T: Iterator<Item = &'t Text<'t>>,
{
    pub fn new(text: T) -> Paragraph<'a, 't, T> {
        Paragraph {
            block: None,
            style: Default::default(),
            wrapping: false,
            raw: false,
            text,
            scroll: 0,
            scroll_from: ScrollFrom::Top,
            alignment: Alignment::Left,
        }
    }

    pub fn block(mut self, block: Block<'a>) -> Paragraph<'a, 't, T> {
        self.block = Some(block);
        self
    }

    pub fn style(mut self, style: Style) -> Paragraph<'a, 't, T> {
        self.style = style;
        self
    }

    pub fn wrap(mut self, flag: bool) -> Paragraph<'a, 't, T> {
        self.wrapping = flag;
        self
    }

    pub fn raw(mut self, flag: bool) -> Paragraph<'a, 't, T> {
        self.raw = flag;
        self
    }

    pub fn scroll(mut self, offset: u16) -> Paragraph<'a, 't, T> {
        self.scroll = offset;
        self
    }

    pub fn scroll_from(mut self, scroll_from: ScrollFrom) -> Paragraph<'a, 't, T> {
        self.scroll_from = scroll_from;
        self
    }

    pub fn alignment(mut self, alignment: Alignment) -> Paragraph<'a, 't, T> {
        self.alignment = alignment;
        self
    }
}

impl<'a, 't, 'b, T> Widget for Paragraph<'a, 't, T>
where
    T: Iterator<Item = &'t Text<'t>>,
{
    fn draw(&mut self, area: Rect, buf: &mut Buffer) {
        let text_area = match self.block {
            Some(ref mut b) => {
                b.draw(area, buf);
                b.inner(area)
            }
            None => area,
        };

        if text_area.height < 1 {
            return;
        }

        self.background(text_area, buf, self.style.bg);

        let style = self.style;
        let mut styled = self.text.by_ref().flat_map(|t| match *t {
            Text::Raw(ref d) => {
                let data: &'t str = d; // coerce to &str
                Either::Left(UnicodeSegmentation::graphemes(data, true).map(|g| Styled(g, style)))
            }
            Text::Styled(ref d, s) => {
                let data: &'t str = d; // coerce to &str
                Either::Right(UnicodeSegmentation::graphemes(data, true).map(move |g| Styled(g, s)))
            }
        });

        let mut line_composer: Box<dyn LineComposer> = if self.wrapping {
            Box::new(WordWrapper::new(&mut styled, text_area.width))
        } else {
            Box::new(LineTruncator::new(&mut styled, text_area.width))
        };
        let mut y = 0;

        match self.scroll_from {
            ScrollFrom::Top => {
                while let Some((current_line, current_line_width)) = line_composer.next_line() {
                    if y >= self.scroll {
                        let mut x =
                            get_line_offset(current_line_width, text_area.width, self.alignment);
                        let buf_y =
                            get_line_vertical_pos(text_area, y, self.scroll, self.scroll_from);
                        for Styled(symbol, style) in current_line {
                            buf.get_mut(text_area.left() + x, buf_y)
                                .set_symbol(symbol)
                                .set_style(*style);
                            x += symbol.width() as u16;
                        }
                    }
                    y += 1;
                    if y >= text_area.height + self.scroll {
                        break;
                    }
                }
            }
            ScrollFrom::Bottom => {
                let mut all_lines = line_composer.collect_lines();
                while let Some((current_line, current_line_width)) = all_lines.pop() {
                    if y >= self.scroll {
                        let mut x =
                            get_line_offset(current_line_width, text_area.width, self.alignment);
                        let buf_y =
                            get_line_vertical_pos(text_area, y, self.scroll, self.scroll_from);
                        for Styled(symbol, style) in current_line {
                            buf.get_mut(text_area.left() + x, buf_y)
                                .set_symbol(symbol)
                                .set_style(style);
                            x += symbol.width() as u16;
                        }
                    }
                    y += 1;
                    if y >= text_area.height + self.scroll {
                        break;
                    }
                }
            }
        }
    }
}
