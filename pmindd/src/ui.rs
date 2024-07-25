use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen, SetSize};
use ratatui::{
    layout::{Alignment, Layout, Rect},
    prelude::{Constraint, CrosstermBackend, Stylize},
    style::{Color, Style},
    text::Text,
    widgets::{Block, BorderType, Cell, HighlightSpacing, Paragraph, Row, Table},
    Frame, Terminal, TerminalOptions, Viewport,
};
use std::{io, panic};

use crate::minder::{PlantMinder, PlantMinderResult};

pub fn render(app: &mut PlantMinder, frame: &mut Frame) {
    let rects = Layout::vertical([Constraint::Min(5), Constraint::Length(3)]).split(frame.size());
    frame.render_widget(
        Paragraph::new("Sensor Data. Press `Esc`, `Ctrl-C` or `q` to stop running.".to_string())
            .block(
                Block::bordered()
                    .title("Plant Minder")
                    .title_alignment(Alignment::Center)
                    .border_type(BorderType::Rounded),
            )
            .style(Style::default().fg(Color::Cyan).bg(Color::Black))
            .centered(),
        rects[1],
    );

    render_table(app, frame, rects[0]);
}

pub fn render_table(app: &mut PlantMinder, f: &mut Frame, area: Rect) {
    let header_style = Style::default().fg(Color::Cyan).bg(Color::Black);

    let header = [
        "Node",
        "Moisture",
        "Temperature",
        "Full Spectrum Light",
        "Lux",
    ]
    .into_iter()
    .map(Cell::from)
    .collect::<Row>()
    .style(header_style)
    .height(1);

    let rows = app.node_data.iter().map(|(addr, data)| {
        let (row_color, text_color) = (Color::Black, Color::Cyan);

        // just take the last pushed value for now
        let (moisture, temperature, full_spectrum, lux) = {
            if data.is_empty() {
                (0, 0.0, 0, 0.0)
            } else {
                let len = data.len();
                (
                    data[len - 1].data.moisture,
                    data[len - 1].data.temperature,
                    data[len - 1].data.full_spectrum,
                    data[len - 1].data.lux,
                )
            }
        };

        Row::new(vec![
            addr.to_string(),
            moisture.to_string(),
            temperature.to_string(),
            full_spectrum.to_string(),
            lux.to_string(),
        ])
        .style(Style::new().fg(text_color).bg(row_color))
        .height(3)
    });

    let bar = " █ ";
    let t = Table::new(
        rows,
        [
            Constraint::Length(50),
            Constraint::Min(10),
            Constraint::Min(10),
            Constraint::Min(10),
            Constraint::Min(10),
        ],
    )
    .header(header)
    .highlight_symbol(Text::from(vec![
        "".into(),
        bar.into(),
        bar.into(),
        "".into(),
    ]))
    .bg(Color::Black)
    .highlight_spacing(HighlightSpacing::Always);
    f.render_stateful_widget(t, area, &mut app.state);
}

#[derive(Debug)]
pub struct Tui {
    terminal: Terminal<CrosstermBackend<io::Stderr>>,
    col: u16,
    row: u16,
}

const TERM_COL: u16 = 110;
const TERM_ROW: u16 = 30;

impl Tui {
    pub fn new() -> Result<Self, std::io::Error> {
        // keep track of prev terminal size before resizing
        let (col, row) = crossterm::terminal::size()?;
        crossterm::execute!(std::io::stdout(), SetSize(TERM_COL, TERM_ROW),)?;

        let options = TerminalOptions {
            viewport: Viewport::Fixed(Rect::new(0, 0, TERM_COL, TERM_ROW)),
        };

        Ok(Self {
            terminal: Terminal::with_options(CrosstermBackend::new(io::stderr()), options)?,
            col,
            row,
        })
    }

    /// Using ratatui template for now
    pub fn init(&mut self) -> PlantMinderResult<()> {
        terminal::enable_raw_mode()?;
        crossterm::execute!(io::stderr(), EnterAlternateScreen)?;

        let panic_hook = panic::take_hook();
        panic::set_hook(Box::new(move |panic| {
            Self::reset().expect("failed to reset the terminal");
            panic_hook(panic);
        }));

        self.terminal.clear()?;
        Ok(())
    }

    pub fn draw(&mut self, app: &mut PlantMinder) -> PlantMinderResult<()> {
        self.terminal.draw(|frame| render(app, frame))?;
        Ok(())
    }

    fn reset() -> PlantMinderResult<()> {
        terminal::disable_raw_mode()?;
        crossterm::execute!(io::stderr(), LeaveAlternateScreen)?;
        Ok(())
    }

    pub fn exit(&mut self) -> PlantMinderResult<()> {
        Self::reset()?;
        crossterm::execute!(
            io::stderr(),
            crossterm::terminal::SetSize(self.col, self.row)
        )?;
        Ok(())
    }
}
