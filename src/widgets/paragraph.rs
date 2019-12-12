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
    scroll_overflow_char: Option<char>,
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
            scroll_overflow_char: None,
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

    pub fn scroll_overflow_char(
        mut self,
        scroll_overflow_char: Option<char>,
    ) -> Paragraph<'a, 't, T> {
        self.scroll_overflow_char = scroll_overflow_char;
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

        let (first_line_index, mut get_next_line): (
            i16,
            Box<dyn FnMut() -> Option<(Vec<Styled<'t>>, u16)>>,
        ) = match self.scroll_from {
            ScrollFrom::Top => {
                let get_next_line = Box::new(|| {
                    line_composer
                        .next_line()
                        .map(|(line, line_width)| (line.to_vec(), line_width))
                });

                (self.scroll as i16, get_next_line)
            }
            ScrollFrom::Bottom => {
                let all_lines = line_composer.collect_lines();
                let num_lines = all_lines.len() as u16;
                let scroll_offset = match self.scroll_overflow_char {
                    // if scroll_overflow is not set, don't
                    // ever scroll beyond the bounds of the content
                    None => {
                        if num_lines <= text_area.height + self.scroll {
                            // prevents us from scrolling up past the
                            // first line, or from scrolling at all
                            // if num_lines <= text_area.height
                            0
                        } else {
                            // default ScrollFrom::Bottom behavior,
                            // scroll == 0 floats content to bottom,
                            // scroll > 0 scrolling up, back in history
                            (num_lines - (text_area.height + self.scroll)) as i16
                        }
                    }
                    // if scroll_overflow is set, scrolling up
                    // back in history past the top of the content results
                    // in a repeated character on each subsequent line
                    // (scroll_overflow_char)
                    Some(_) => {
                        if num_lines <= text_area.height {
                            // if content doesn't fill the text_area,
                            // scrolling should be reverse of normal
                            // behavior
                            -(self.scroll as i16)
                        } else {
                            // default ScrollFrom::Bottom behavior,
                            // scroll == 0 floats content to bottom,
                            // scroll > 0 scrolling up, back in history
                            num_lines as i16 - (text_area.height + self.scroll) as i16
                        }
                    }
                };

                let mut all_lines_iter = all_lines.into_iter();
                let get_next_line = Box::new(move || all_lines_iter.next());

                (scroll_offset, get_next_line)
            }
        };

        let mut current_line_index = 0;

        for y in 0..text_area.height {
            if (y as i16) < -first_line_index {
                let overflow_char = self.scroll_overflow_char.unwrap();
                buf.get_mut(text_area.left(), text_area.top() + y as u16)
                    .set_symbol(&overflow_char.to_string());
            } else {
                while let Some((current_line, current_line_width)) = get_next_line() {
                    if current_line_index >= first_line_index {
                        let mut x =
                            get_line_offset(current_line_width, text_area.width, self.alignment);

                        for Styled(symbol, style) in current_line {
                            buf.get_mut(text_area.left() + x, text_area.top() + y)
                                .set_symbol(symbol)
                                .set_style(style);
                            x += symbol.width() as u16;
                        }
                        current_line_index += 1;
                        break;
                    } else {
                        current_line_index += 1;
                    }
                }
            }
        }
    }
}
