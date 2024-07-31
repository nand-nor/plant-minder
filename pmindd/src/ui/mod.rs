use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::Block,
};

mod history;
pub use history::{HistoryView, NodeHistory, NodeHistoryState};

pub struct Background;

impl ratatui::widgets::Widget for Background {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Block::new()
            .style(Style::new().fg(Color::Cyan).bg(Color::Cyan))
            .render(area, buf);
    }
}
